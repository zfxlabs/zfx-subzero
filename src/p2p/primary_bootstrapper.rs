//! Bootstrapper responsible for bootstrapping the primary network and chain.
//!
//! The `PrimaryBootstrapper` is used to initialise the network and the primary chain in order to
//! instantiate the initial validator set required for subsequent network bootstraps. Subsequent
//! bootstrappers are expected to use trusted validator sets derived from the primary network state.

use crate::alpha::{Alpha, LastCellId, ValidatorSet};
use crate::cell::CellId;
use crate::ice::Ice;
use crate::server::{InitIce, Router, TransitionReady};

use super::prelude::*;

use super::linear_backoff::{LinearBackoff, Start};
use super::peer_bootstrapper::ReceivePeerSet;
use super::primary_synchroniser::PrimarySynchroniser;

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct PrimaryBootstrapper {
    /// The connection upgrader.
    upgrader: Arc<dyn Upgrader>,
    /// The metadata of this peer.
    self_peer: PeerMetadata,
    /// The `Id` of the chain being bootstrapped.
    chain_id: Id,
    /// A trusted set of bootstrap peers for bootstrapping the chain.
    bootstrap_peers: HashSet<PeerMetadata>,
    /// The number of peers required to bootstrap the chain.
    bootstrap_peer_lim: usize,
    /// Router which handles external requests.
    router_address: Addr<Router>,
    /// The `alpha` (primary chain protocol) address.
    alpha_address: Addr<Alpha>,
}

impl PrimaryBootstrapper {
    pub fn new(
        upgrader: Arc<dyn Upgrader>,
        self_peer: PeerMetadata,
        chain_id: Id,
        bootstrap_peer_lim: usize,
        router_address: Addr<Router>,
        alpha_address: Addr<Alpha>,
    ) -> Self {
        PrimaryBootstrapper {
            upgrader,
            self_peer,
            chain_id,
            bootstrap_peers: HashSet::default(),
            bootstrap_peer_lim,
            router_address,
            alpha_address,
        }
    }

    /// Inserts a new bootstrap peer and returns `Some(_)` when the bootstrap peer limit has been
    /// reached, otherwise `None` is returned.
    pub fn insert_bootstrap_peer(
        &mut self,
        peer_meta: PeerMetadata,
    ) -> Option<HashSet<PeerMetadata>> {
        if self.bootstrap_peers.len() >= self.bootstrap_peer_lim {
            return Some(self.bootstrap_peers.clone());
        } else {
            if let true = self.bootstrap_peers.insert(peer_meta) {
                if self.bootstrap_peers.len() >= self.bootstrap_peer_lim {
                    Some(self.bootstrap_peers.clone())
                } else {
                    None
                }
            } else {
                None
            }
        }
    }
}

impl Actor for PrimaryBootstrapper {
    type Context = Context<Self>;
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct ReceivePrimaryBootstrap {
    pub chain: Id,
    pub peers: HashSet<PeerMetadata>,
}

impl Handler<ReceivePeerSet> for PrimaryBootstrapper {
    type Result = ();

    fn handle(&mut self, msg: ReceivePeerSet, ctx: &mut Context<Self>) -> Self::Result {
        info!("received peer_set");
        // Collects the peers which support the primary chain and tries to insert them into the
        // primary bootstrappers
        let mut primary_peers = HashSet::new();
        for peer in msg.peer_set.iter().cloned() {
            if peer.chains.contains(&self.chain_id) {
                primary_peers.insert(peer.clone());
                match self.insert_bootstrap_peer(peer.clone()) {
                    // If the primary chain has enough peers, start bootstrapping the primary
                    // chain
                    Some(peers) => {
                        let arbiter = Arbiter::new();
                        let alpha_address = self.alpha_address.clone();
                        let upgrader = self.upgrader.clone();
                        let self_peer = self.self_peer.clone();
                        let sync_recipient = ctx.address().recipient().clone();
                        arbiter.spawn(async move {
                            match alpha_address.send(LastCellId).await.unwrap() {
                                Ok(last_cell_id) => {
                                    // Synchronise the chain state according to the trusted peers
                                    info!("bootstrapped: initialising primary synchroniser");
                                    let primary_synchroniser_address = PrimarySynchroniser::new(
                                        upgrader.clone(),
                                        self_peer.clone(),
                                        last_cell_id,
                                        primary_peers,
                                        sync_recipient,
                                    )
                                    .start();

                                    info!("primary sync backoff delay = 10s");
                                    let backoff = LinearBackoff::new(
                                        primary_synchroniser_address.recipient(),
                                        Duration::from_millis(10000),
                                    )
                                    .start();
                                    let () = backoff.do_send(Start);
                                }
                                Err(err) => error!("{:?}", err),
                            }
                        });
                        return;
                    }
                    None => continue,
                }
            } else {
                error!("error: database unopened, skipping primary bootstrapper")
            }
        }
    }
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct ReceiveSynchronised {
    pub last_cell_id: CellId,
}

impl Handler<ReceiveSynchronised> for PrimaryBootstrapper {
    type Result = ();

    fn handle(&mut self, msg: ReceiveSynchronised, ctx: &mut Context<Self>) -> Self::Result {
        info!("--- bootststrap synchronisation complete ---");
        info!("fetching latest validator set from `alpha`");
        let arbiter = Arbiter::new();
        let upgrader = self.upgrader.clone();
        let self_peer = self.self_peer.clone();
        let bootstrap_peers = self.bootstrap_peers.clone();
        let router_address = self.router_address.clone();
        let alpha_address = self.alpha_address.clone();
        arbiter.spawn(async move {
            let () = router_address.send(TransitionReady).await.unwrap();
            match alpha_address.send(ValidatorSet { cell_id: msg.last_cell_id }).await.unwrap() {
                Ok(alpha_validators) => {
                    info!("received validators =>\n{:?}", alpha_validators.clone());
                    // if should_spawn_ice {
                    let mut validators = vec![];
                    for (id, capacity) in alpha_validators.iter() {
                        for peer_meta in bootstrap_peers.iter() {
                            if *id == peer_meta.id {
                                validators.push((peer_meta.clone(), *capacity));
                                break;
                            }
                        }
                    }
                    info!("initialising `ice` with delay = 3s");
                    let ice_address = Ice::new(upgrader, self_peer, validators).start();
                    let () =
                        router_address.send(InitIce { addr: ice_address.clone() }).await.unwrap();
                    let backoff =
                        LinearBackoff::new(ice_address.recipient(), Duration::from_millis(3000))
                            .start();
                    backoff.do_send(Start);
                }
                Err(err) => error!("{:?}", err),
            }
        });
    }
}

use super::prelude::*;

use super::chain_bootstrapper::{ChainBootstrapper, ReceiveBootstrapped};
use super::peer_bootstrapper::ReceivePeerVec;

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

/// Bootstraps a set of chains within a network configuration.
pub struct NetworkBootstrapper {
    /// The chain ids of the chains this network is subscribed to.
    chains: HashSet<Id>,
    /// Map of chain id to bootstrap peers which can be used to bootstrap a chain.
    chain_bootstrap_peers: HashMap<Id, HashSet<PeerMetadata>>,
    /// Map of chain id to chain bootstrappers.
    chain_bootstrappers: HashMap<Id, Addr<ChainBootstrapper>>,
    /// The limit to the number of peers required to bootstrap the chain.
    chain_bootstrap_peer_lim: usize,
    /// The number of chains which are bootstrapped.
    chain_bootstraps: usize,
}

impl NetworkBootstrapper {
    pub fn new(chains: Vec<Id>, chain_bootstrap_peer_lim: usize) -> Self {
        NetworkBootstrapper {
            chains: chains.iter().cloned().collect::<HashSet<Id>>(),
            chain_bootstrap_peers: HashMap::default(),
            chain_bootstrappers: HashMap::default(),
            chain_bootstrap_peer_lim,
            chain_bootstraps: 0,
        }
    }

    pub fn insert_bootstrap_peer(
        &mut self,
        chain_id: Id,
        peer_meta: PeerMetadata,
    ) -> Option<HashSet<PeerMetadata>> {
        match self.chain_bootstrap_peers.entry(chain_id) {
            Entry::Occupied(mut o) => {
                let hs = o.get_mut();
                hs.insert(peer_meta);
                if hs.len() >= self.chain_bootstrap_peer_lim {
                    let peers = hs.clone();
                    *hs = HashSet::new();
                    Some(peers)
                } else {
                    None
                }
            }
            Entry::Vacant(mut v) => {
                let mut hs = HashSet::new();
                hs.insert(peer_meta);
                v.insert(hs);
                None
            }
        }
    }
}

impl Actor for NetworkBootstrapper {
    type Context = Context<Self>;

    fn stopped(&mut self, ctx: &mut Context<Self>) {
        info!("stopped");
    }
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct ReceiveBootstrapPeers {
    pub chain: Id,
    pub peers: Vec<PeerMetadata>,
}

impl Handler<ReceivePeerVec> for NetworkBootstrapper {
    type Result = ();

    fn handle(&mut self, msg: ReceivePeerVec, ctx: &mut Context<Self>) -> Self::Result {
        info!("received peer vec");
        for peer_meta in msg.v.iter() {
            let peer_chains = peer_meta.clone().chains.iter().cloned().collect::<HashSet<Id>>();
            let supported_chains: HashSet<Id> =
                self.chains.intersection(&peer_chains).cloned().collect();
            if supported_chains.len() > 0 {
                // Add the peer metadata according to the supported chains in the set
                for chain_id in supported_chains.iter().cloned() {
                    match self.insert_bootstrap_peer(chain_id, peer_meta.clone()) {
                        // If a chain is ready, start the chain bootstrapper
                        Some(peers) => {
                            if let Some(_) = self.chain_bootstrappers.get(&chain_id) {
                                warn!("chain {:?} is already bootstrapping", chain_id);
                            } else {
                                let self_recipient = ctx.address().recipient();
                                let chain_bootstrapper_address =
                                    ChainBootstrapper::new(chain_id.clone(), peers, self_recipient)
                                        .start();
                                self.chain_bootstrappers
                                    .insert(chain_id, chain_bootstrapper_address);
                            }
                        }
                        None => (),
                    }
                }
            } else {
                warn!("rejected peer {:?} - no chains are supported", peer_meta);
            }
        }
    }
}

impl Handler<ReceiveBootstrapped> for NetworkBootstrapper {
    type Result = ();

    fn handle(&mut self, msg: ReceiveBootstrapped, ctx: &mut Context<Self>) -> Self::Result {
        ()
    }
}

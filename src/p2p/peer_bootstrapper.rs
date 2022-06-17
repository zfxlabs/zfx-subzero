//! The `PeerBootstrapper` receives `Execute` messages periodically and multicasts `Version` messages
//! to neighboring peers. Peers share known `metadata` with one another via the handshake, which also
//! serves to identify nodes according to their `id`s.
//!
//! Once a vector of `PeerMetadata` is received the `PeerBootstrapper` forwards the peers to a
//! recipient for further processing.
//!
//! Note: The peer bootstrapper can be improved by using e.g. `gradecast` for adding byzantine fault
//! tolerance.

use super::prelude::*;

use crate::message::{Version, CURRENT_VERSION};
use crate::protocol::network::{NetworkRequest, NetworkResponse};

use super::linear_backoff::Execute;
use super::multicast::{Multicast, MulticastRequest, MulticastResult};
use super::response_handler::ResponseHandler;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// The limit of a peer vector in terms of length received from another peer.
const RECEIVED_PEER_VEC_LIM: usize = 124;

pub struct PeerBootstrapper {
    /// Metadata pertaining to this peer.
    self_peer_meta: PeerMetadata,
    /// An initial trusted set of remote peers.
    trusted_peers: Vec<PeerMetadata>,
    /// The amount of peers which are allowed to be discovered from other trusted peers.
    trusted_peer_discovery_limit: usize,
    /// The amount of trusted peers which have been discovered from receiving peer metadata.
    trusted_peers_discovered: usize,
    /// A connection upgrader (e.g. upgrade plain TCP / upgrade TLS).
    upgrader: Arc<dyn Upgrader>,
    /// The recipient `Actor` of the `peer_set` (`HashSet<PeerMetada>`).
    peer_set_recipient: Recipient<ReceivePeerSet>,
    /// Extent of time which a single send is allowed to take.
    timeout: Duration,
    /// The current multicast iteration.
    iteration: usize,
    /// The number of retries to perform.
    iteration_limit: usize,
    /// Whether the peer bootstrapper has finished.
    finished: bool,
}

impl PeerBootstrapper {
    pub fn new(
        upgrader: Arc<dyn Upgrader>,
        self_peer_meta: PeerMetadata,
        trusted_peers: Vec<PeerMetadata>,
        trusted_peer_discovery_limit: usize,
        iteration_limit: usize,
        peer_set_recipient: Recipient<ReceivePeerSet>,
        timeout: Duration,
    ) -> Self {
        PeerBootstrapper {
            upgrader,
            self_peer_meta,
            trusted_peers,
            trusted_peer_discovery_limit,
            trusted_peers_discovered: 0,
            peer_set_recipient,
            timeout,
            iteration: 0,
            iteration_limit,
            finished: false,
        }
    }
}

impl Actor for PeerBootstrapper {
    type Context = Context<Self>;
}

impl Handler<Execute> for PeerBootstrapper {
    type Result = ResponseFuture<bool>;

    fn handle(&mut self, msg: Execute, ctx: &mut Context<Self>) -> Self::Result {
        let self_recipient = ctx.address().recipient().clone();
        let peer_set = self.trusted_peers.iter().cloned().collect::<HashSet<PeerMetadata>>();
        let multicast = Multicast::<NetworkResponse>::new(
            self.upgrader.clone(),
            peer_set.clone(),
            self_recipient,
            self.timeout.clone(),
        )
        .start();
        let request = NetworkRequest::Version(Version::new(self.self_peer_meta.clone(), peer_set));
        if !self.finished {
            if self.iteration > self.iteration_limit {
                warn!("multicast repeated beyond the iteration limit");
                Box::pin(async { true })
            } else if self.iteration == self.iteration_limit {
                info!("reached iteration limit");
                self.iteration += 1;
                self.finished = true;
                Box::pin(async { true })
            } else {
                self.iteration += 1;
                Box::pin(async move {
                    info!("multicasting {:?}", request.clone());
                    let _ = multicast.send(MulticastRequest { request }).await.unwrap();
                    false
                })
            }
        } else {
            Box::pin(async { false })
        }
    }
}

impl Handler<MulticastResult<NetworkResponse>> for PeerBootstrapper {
    type Result = ResponseFuture<()>;

    fn handle(
        &mut self,
        msg: MulticastResult<NetworkResponse>,
        ctx: &mut Context<Self>,
    ) -> Self::Result {
        info!("multicast result: {:?}", msg);
        // Save the trusted peers length prior to discovery since it may extend the vector
        let undiscovered_trusted_peer_len = self.trusted_peers.len();
        // Peers are accumulated in a `HashSet` to prevent duplicates
        let mut peer_set = HashSet::default();
        for response in msg.result.iter().cloned() {
            match response {
                NetworkResponse::VersionAck(version_ack) => {
                    if version_ack.version == CURRENT_VERSION {
                        // Peers discover new peers based on version ack responses
                        if self.trusted_peers_discovered < self.trusted_peer_discovery_limit {
                            for peer_meta in version_ack.peer_set.iter().cloned() {
                                if peer_meta != self.self_peer_meta {
                                    self.trusted_peers.push(peer_meta);
                                    self.trusted_peers_discovered += 1;
                                }
                            }
                        }
                        // The responding peer is added to the final peer set
                        let _ = peer_set.insert(version_ack.peer);
                        ()
                    }
                }
                _ => (),
            }
        }
        if peer_set.len() == undiscovered_trusted_peer_len {
            self.finished = true;
        }
        let peer_set_recipient = self.peer_set_recipient.clone();
        Box::pin(async move { peer_set_recipient.send(ReceivePeerSet { peer_set }).await.unwrap() })
    }
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct ReceivePeerSet {
    pub peer_set: HashSet<PeerMetadata>,
}

// A `VersionAck` is reponded when a `Version` request is made to a peer. The `VersionAckHandler`
// sends the peers response to the `PeerBootstrapper` such that the contained metadata may
// be aggregated.

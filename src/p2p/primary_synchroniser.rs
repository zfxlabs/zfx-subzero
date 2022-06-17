//! The `PrimarySynchroniser` establishes which cell is the last cell by sampling a set of trusted
//! peers and obtaining a network quorum based on a supplied threshold. This threshold is unrelated
//! to consensus and can be arbitrarily specified.
//!
//! The `PrimarySynchroniser` is assumed to be bootstrapped with whitelisted participants which have
//! a common genesis - no further security checking is done at this stage. The `PrimarySynchroniser`
//! is constructed with the `last_cell_id` found within this peers current state, which is used to
//! acquire missing future cells from nodes in case there is missing data and is used to determine
//! when the synchronisation process should halt.
//!
//! Note that if the `PrimarySynchroniser` is supplied with peers which are not trusted, arbitrary
//! chain data may be downloaded. In an eclipse attack, malicious peers can fool another peer into
//! believing it is on a valid network which is separate from the real one. Thus it is of paramount
//! importance from the point of view of security to have a valid set of active validators to
//! bootstrap from and for the routes to be unimpeded.

use super::linear_backoff::Execute;
use super::peer_meta::PeerMetadata;
use super::prelude::*;
use super::primary_bootstrapper::ReceiveSynchronised;
use super::response_handler::ResponseHandler;
use super::sender::{multicast, Sender};
use crate::cell::CellId;
use crate::message::LastCellId;
use crate::protocol::graph::{GraphRequest, GraphResponse};
use crate::protocol::network::{NetworkRequest, NetworkResponse};

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct PrimarySynchroniser {
    /// The current connection upgrader.
    upgrader: Arc<dyn Upgrader>,
    /// The metadata of this peer.
    self_peer: PeerMetadata,
    /// The last cell `id` of this peer.
    self_last_cell_id: CellId,
    /// A vec of trusted peers which are subscribed to the primary network.
    primary_peers: HashSet<PeerMetadata>,
    /// The longest time to wait for a multicast response.
    delta: Duration,
    /// The threshold of last cell ids required to make a decision.
    primary_sync_threshold: usize,
    /// The number of last cells to accumulate prior to making a decision.
    quorum_lim: usize,
    /// The accumulated cell ids received by this actor.
    quorum: HashMap<PeerMetadata, CellId>,
    /// Decisions over an accepted last cell id.
    decisions: HashMap<CellId, usize>,
    /// Whether the `PrimarySynchroniser` has finished.
    complete: Arc<AtomicBool>,
    /// The recipient of `ReceiveSynchronised` when the primary synchroniser has finished.
    sync_recipient: Recipient<ReceiveSynchronised>,
}

impl PrimarySynchroniser {
    pub fn new(
        upgrader: Arc<dyn Upgrader>,
        self_peer: PeerMetadata,
        self_last_cell_id: CellId,
        primary_peers: HashSet<PeerMetadata>,
        sync_recipient: Recipient<ReceiveSynchronised>,
    ) -> Self {
        PrimarySynchroniser {
            upgrader,
            self_peer,
            self_last_cell_id,
            primary_peers,
            delta: Duration::from_millis(1000),
            primary_sync_threshold: 2,
            quorum_lim: 2,
            quorum: HashMap::default(),
            decisions: HashMap::default(),
            complete: Arc::new(AtomicBool::new(false)),
            sync_recipient,
        }
    }

    pub fn set_delta(&mut self, delta: Duration) {
        self.delta = delta;
    }

    pub fn set_primary_sync_threshold(&mut self, threshold: usize) {
        self.primary_sync_threshold = threshold;
    }
}

impl Actor for PrimarySynchroniser {
    type Context = Context<Self>;
}

// Expected to be controlled by a `Backoff` actor.
impl Handler<Execute> for PrimarySynchroniser {
    type Result = ResponseActFuture<Self, bool>;

    fn handle(&mut self, msg: Execute, ctx: &mut Context<Self>) -> Self::Result {
        let self_recipient = ctx.address().recipient().clone();
        let last_cell_id_handler = LastCellIdHandler::new(self_recipient);
        let sender_addr = Sender::new(self.upgrader.clone(), last_cell_id_handler).start();
        let request = NetworkRequest::GraphRequest(GraphRequest::LastCellId(LastCellId::new(
            self.self_peer.clone(),
        )));
        let multicast_fut = multicast::<NetworkRequest, NetworkResponse>(
            sender_addr,
            self.primary_peers.clone(),
            request,
            self.delta.clone(),
        );
        let multicast_wrapped = actix::fut::wrap_future::<_, Self>(multicast_fut);
        Box::pin(
            multicast_wrapped
                .map(move |responses, actor, ctx| actor.complete.load(Ordering::Relaxed)),
        )
    }
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct ReceiveLastCellId {
    pub peer_meta: PeerMetadata,
    pub last_cell_id: CellId,
}

impl Handler<ReceiveLastCellId> for PrimarySynchroniser {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: ReceiveLastCellId, ctx: &mut Context<Self>) -> Self::Result {
        info!("ReceiveLastCellId({:?}, {:?})", msg.peer_meta.ip.clone(), msg.last_cell_id.clone());

        // If the `primary_synchroniser` is complete then we no longer should be receiving
        // `last_cell_id`s in this actor.
        let complete = self.complete.clone().load(Ordering::Relaxed);
        if complete {
            return Box::pin(async {});
        }

        // If a quorum is complete then complete the first stage of the synchronisation protocol.
        if self.quorum.len() >= self.quorum_lim {
            let mut decision = None;
            for (d, x) in self.decisions.iter() {
                if *x >= self.primary_sync_threshold {
                    info!(">>> primary_sync_threshold({:?}) <<<", d.clone());
                    decision = Some(d);
                    break;
                }
            }
            match decision {
                Some(d) => {
                    // If the `last_cell_id` corroborates, then we are done
                    if self.self_last_cell_id == *d {
                        info!("cell synchronisation complete");
                        // Ensures that the actor no longer receives backoff messages.
                        self.complete.store(true, Ordering::Relaxed);

                        // Alert the primary bootstrapper that we are done.
                        let sync_recipient = self.sync_recipient.clone();
                        let last_cell_id = self.self_last_cell_id.clone();
                        Box::pin(async move {
                            let () = sync_recipient
                                .send(ReceiveSynchronised { last_cell_id })
                                .await
                                .unwrap();
                        })
                    } else {
                        // TODO: Ancestor based synchronisation
                        // If the decision is not the same as this `last_cell_id` then:
                        //   1. Fetch all the peer metadatas whose last cell id is `*d`
                        //   2. Try to fetch all ancestors of the `last_cell_id` from the peer.
                        info!("TODO: sync to latest cell id");
                        Box::pin(async {})
                    }
                }
                // Otherwise try again after some delay
                None => Box::pin(async {}),
            }
        } else {
            // If a quorum is incomplete then accumulate last cell ids and update the decisions.
            let _ = self.quorum.insert(msg.peer_meta.clone(), msg.last_cell_id.clone());
            self.decisions.entry(msg.last_cell_id.clone()).and_modify(|x| *x += 1).or_insert(1);
            Box::pin(async move {})
        }
    }
}

// Handles incoming cell ids after being requested
pub struct LastCellIdHandler {
    recipient: Recipient<ReceiveLastCellId>,
}

impl LastCellIdHandler {
    pub fn new(
        recipient: Recipient<ReceiveLastCellId>,
    ) -> Arc<dyn ResponseHandler<NetworkResponse>> {
        Arc::new(LastCellIdHandler { recipient })
    }
}

// A `LastAck` is responded when a `GetLast` request is made to a peer. The `LastHandler` sends the
// peers responses to the `PrimarySynchroniser` which aggregates the last cell ids in order to
// establish what the network believes is the last accepted cell.
impl ResponseHandler<NetworkResponse> for LastCellIdHandler {
    fn handle_response(
        &self,
        response: NetworkResponse,
    ) -> Pin<Box<dyn Future<Output = Result<()>>>> {
        let recipient = self.recipient.clone();
        match response {
            NetworkResponse::GraphResponse(graph_response) => match graph_response {
                GraphResponse::LastCellIdAck(last_cell_id_ack) => Box::pin(async move {
                    recipient
                        .send(ReceiveLastCellId {
                            peer_meta: last_cell_id_ack.peer,
                            last_cell_id: last_cell_id_ack.last_cell_id,
                        })
                        .await
                        .map_err(|err| err.into())
                }),
                _ => Box::pin(async { Err(Error::InvalidResponse) }),
            },
            _ => Box::pin(async { Err(Error::InvalidResponse) }),
        }
    }
}

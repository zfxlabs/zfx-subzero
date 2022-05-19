use super::prelude::*;

use crate::version::{self, Version};

use super::linear_backoff::Execute;
use super::sender::{multicast, Sender};

use super::response_handler::ResponseHandler;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// The limit of a peer vector in terms of length received from another peer.
const RECEIVED_PEER_VEC_LIM: usize = 124;

/// The `PeerBootstrapper` receives `Execute` messages periodically and multicasts `Version`
/// messages to neighboring peers. Peers share known `metadata` with one another via the
/// handshake, which also serves to identify nodes according to their `id`s.
///
/// Once a vector of `PeerMetadata` is received the `PeerBootstrapper` forwards the peers
/// to a recipient for further processing.
pub struct PeerBootstrapper {
    /// Metadata pertaining to this peer.
    local_peer_meta: PeerMetadata,
    /// An initial trusted set of remote peers.
    remote_peer_metas: Vec<PeerMetadata>,
    /// A connection upgrader (e.g. upgrade plain TCP / upgrade TLS).
    upgrader: Arc<dyn Upgrader>,
    /// The recipient `Actor` of `Vec<Peer>`s.
    peer_vec_recipient: Recipient<ReceivePeerVec>,
    /// The maximum length of a peer vector.
    peer_vec_lim: usize,
    /// The maximum number of peer vectors to share.
    peer_vec_share_lim: usize,
    /// The current peer vector.
    current_peer_vec: Vec<PeerMetadata>,
    /// The epoch duration.
    delta: Duration,
    /// The number of peer vectors sent in total.
    sent_peer_vecs: Arc<AtomicUsize>,
    /// Whether the `PeerBootstrapper` is bootstrapped.
    bootstrapped: Arc<AtomicBool>,
}

impl PeerBootstrapper {
    pub fn new(
        upgrader: Arc<dyn Upgrader>,
        local_peer_meta: PeerMetadata,
        remote_peer_metas: Vec<PeerMetadata>,
        peer_vec_recipient: Recipient<ReceivePeerVec>,
        peer_vec_lim: usize,
        peer_vec_share_lim: usize,
        delta: Duration,
    ) -> Self {
        PeerBootstrapper {
            upgrader,
            local_peer_meta,
            remote_peer_metas,
            peer_vec_recipient,
            peer_vec_lim,
            peer_vec_share_lim,
            current_peer_vec: vec![],
            delta,
            sent_peer_vecs: Arc::new(AtomicUsize::new(0)),
            bootstrapped: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Actor for PeerBootstrapper {
    type Context = Context<Self>;

    fn stopped(&mut self, ctx: &mut Context<Self>) {
        info!("stopped");
    }
}

impl Handler<Execute> for PeerBootstrapper {
    type Result = ResponseActFuture<Self, bool>;

    fn handle(&mut self, msg: Execute, ctx: &mut Context<Self>) -> Self::Result {
        // The peer handler uses this actor and sends `ReceivePeer` to it once a peer has
        // been handled.
        let self_recipient = ctx.address().recipient().clone();
        let peer_handler = PeerHandler::new(self_recipient);
        let sender_address = Sender::new(self.upgrader.clone(), peer_handler).start();
        let request = Request::Version(Version::new(self.local_peer_meta.clone()));
        let multicast_fut =
            multicast(sender_address, self.remote_peer_metas.clone(), request, self.delta.clone());
        let multicast_wrapped = actix::fut::wrap_future::<_, Self>(multicast_fut);
        Box::pin(
            multicast_wrapped
                .map(move |responses, actor, ctx| actor.bootstrapped.load(Ordering::Relaxed)),
        )
    }
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct ReceivePeer {
    pub peer_meta: PeerMetadata,
    pub peer_list: Vec<PeerMetadata>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct ReceivePeerVec {
    pub v: Vec<PeerMetadata>,
}

impl Handler<ReceivePeer> for PeerBootstrapper {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: ReceivePeer, ctx: &mut Context<Self>) -> Self::Result {
        // TODO: check the peer metadata more thoroughly
        info!("current_peer_vec.len() = {}", self.current_peer_vec.len());
        if self.current_peer_vec.len() >= self.peer_vec_lim {
            let v = self.current_peer_vec.clone();
            self.current_peer_vec = vec![];
            let peer_vec_recipient = self.peer_vec_recipient.clone();
            let peer_vec_lim = self.peer_vec_lim.clone();
            let sent_peer_vecs = self.sent_peer_vecs.clone();
            let bootstrapped = self.bootstrapped.clone();
            Box::pin(async move {
                peer_vec_recipient.send(ReceivePeerVec { v }).await;
                let n_sent_peer_vecs = sent_peer_vecs.load(Ordering::Relaxed);
                sent_peer_vecs.store(n_sent_peer_vecs + 1, Ordering::Relaxed);
                if n_sent_peer_vecs + 1 >= peer_vec_lim {
                    info!("bootstrapped");
                    bootstrapped.store(true, Ordering::Relaxed);
                }
            })
        } else {
            self.current_peer_vec.push(msg.peer_meta);
            Box::pin(async {})
        }
    }
}

pub struct PeerHandler {
    recipient: Recipient<ReceivePeer>,
}

impl PeerHandler {
    pub fn new(recipient: Recipient<ReceivePeer>) -> Arc<dyn ResponseHandler> {
        let peer_handler = PeerHandler { recipient };
        Arc::new(peer_handler)
    }
}

// A `VersionAck` is reponded when a `Version` request is made to a peer. The `PeerHandler`
// sends the peers response to the `PeerBootstrapper` such that the contained metadata may
// be aggregated.
impl ResponseHandler for PeerHandler {
    fn handle_response(&self, response: Response) -> Pin<Box<dyn Future<Output = Result<()>>>> {
        let recipient = self.recipient.clone();
        match response {
            Response::VersionAck(version_ack) => Box::pin(async move {
                if version_ack.version == version::CURRENT_VERSION {
                    if version_ack.peer_list.len() > RECEIVED_PEER_VEC_LIM {
                        Err(Error::PeerListOverflow)
                    } else {
                        recipient
                            .send(ReceivePeer {
                                peer_meta: version_ack.remote_peer_meta,
                                peer_list: version_ack.peer_list.clone(),
                            })
                            .await
                            .map_err(|err| err.into())
                    }
                } else {
                    Err(Error::IncompatibleVersion)
                }
            }),
            _ => Box::pin(async { Err(Error::InvalidResponse) }),
        }
    }
}

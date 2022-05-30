use super::prelude::*;

use crate::version::{self, Version};

use super::linear_backoff::Execute;
use super::sender::{multicast, Sender};

use super::response_handler::ResponseHandler;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::collections::HashSet;

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
    self_peer_meta: PeerMetadata,
    /// An initial trusted set of remote peers.
    trusted_peers: Vec<PeerMetadata>,
    /// A connection upgrader (e.g. upgrade plain TCP / upgrade TLS).
    upgrader: Arc<dyn Upgrader>,
    /// The recipient `Actor` of the `peer_set` (`HashSet<PeerMetada>`).
    peer_set_recipient: Recipient<ReceivePeerSet>,
    /// The maximum length of a peer vector.
    peer_set_lim: usize,
    /// The maximum number of peer vectors to share.
    peer_set_share_lim: usize,
    /// The current peer set.
    current_peer_set: HashSet<PeerMetadata>,
    /// The epoch duration.
    delta: Duration,
    /// The number of peer vectors sent in total.
    sent_peer_sets: Arc<AtomicUsize>,
    /// Whether the `PeerBootstrapper` is bootstrapped.
    bootstrapped: Arc<AtomicBool>,
}

impl PeerBootstrapper {
    pub fn new(
        upgrader: Arc<dyn Upgrader>,
        self_peer_meta: PeerMetadata,
        trusted_peers: Vec<PeerMetadata>,
        peer_set_recipient: Recipient<ReceivePeerSet>,
        peer_set_lim: usize,
        peer_set_share_lim: usize,
        delta: Duration,
    ) -> Self {
        PeerBootstrapper {
            upgrader,
            self_peer_meta,
	    trusted_peers,
            peer_set_recipient,
            peer_set_lim,
            peer_set_share_lim,
            current_peer_set: HashSet::default(),
            delta,
            sent_peer_sets: Arc::new(AtomicUsize::new(0)),
            bootstrapped: Arc::new(AtomicBool::new(false)),
        }
    }

    fn update_peer_set(&mut self, new_peer_set: HashSet<PeerMetadata>) -> HashSet<PeerMetadata> {
	let old_peer_set = self.current_peer_set.clone();
	self.current_peer_set = new_peer_set;
	old_peer_set
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
	let peer_set = self.trusted_peers.iter().cloned().collect::<HashSet<PeerMetadata>>();
        let request = Request::Version(Version::new(self.self_peer_meta.clone(), peer_set));
        let multicast_fut =
            multicast(sender_address, self.trusted_peers.clone(), request, self.delta.clone());
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
    pub peer_set: HashSet<PeerMetadata>,
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct ReceivePeerSet {
    pub peer_set: HashSet<PeerMetadata>,
}

impl Handler<ReceivePeer> for PeerBootstrapper {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: ReceivePeer, ctx: &mut Context<Self>) -> Self::Result {
        // TODO: check the peer metadata more thoroughly

	// If the `peer_bootstrapper` has already completed the bootstrap, ignore messages
	let bootstrapped = self.bootstrapped.clone().load(Ordering::Relaxed);
	if bootstrapped {
	    return Box::pin(async {});
	}

        if self.current_peer_set.len() >= self.peer_set_lim {
	    let peer_set = self.update_peer_set(HashSet::default());
            let peer_set_recipient = self.peer_set_recipient.clone();
            let peer_set_lim = self.peer_set_lim.clone();
            let sent_peer_sets = self.sent_peer_sets.clone();
            let bootstrapped = self.bootstrapped.clone();
            Box::pin(async move {
                peer_set_recipient.send(ReceivePeerSet { peer_set }).await;
                let n_sent_peer_sets = sent_peer_sets.load(Ordering::Relaxed);
                sent_peer_sets.store(n_sent_peer_sets + 1, Ordering::Relaxed);
                if n_sent_peer_sets + 1 >= peer_set_lim {
                    info!("bootstrapped");
                    bootstrapped.store(true, Ordering::Relaxed);
                }
            })
        } else {
            self.current_peer_set.insert(msg.peer_meta);
            Box::pin(async {})
        }
    }
}

pub struct PeerHandler {
    recipient: Recipient<ReceivePeer>,
}

impl PeerHandler {
    pub fn new(recipient: Recipient<ReceivePeer>) -> Arc<dyn ResponseHandler> {
        Arc::new(PeerHandler { recipient })
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
                    if version_ack.peer_set.len() > RECEIVED_PEER_VEC_LIM {
                        Err(Error::PeerListOverflow)
                    } else {
                        recipient
                            .send(ReceivePeer {
                                peer_meta: version_ack.peer,
                                peer_set: version_ack.peer_set,
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

use super::prelude::*;

use crate::version::{self, Version};

use super::linear_backoff::Execute;
use super::sender::{multicast, Sender};

use super::response_handler::ResponseHandler;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const MAX_PEER_LIST: usize = 124;

pub struct PeerBootstrapper {
    upgrader: Arc<dyn Upgrader>,
    local_peer_meta: PeerMetadata,
    remote_peer_metas: Vec<PeerMetadata>,
    peer_group_recipient: Recipient<ReceivePeerGroup>,
    peer_group_size: usize,
    peer_group_limit: usize,
    current_peer_group: Vec<PeerMetadata>,
    delta: Duration,
    sent_peer_groups: Arc<AtomicUsize>,
    bootstrapped: Arc<AtomicBool>,
}

impl PeerBootstrapper {
    pub fn new(
        upgrader: Arc<dyn Upgrader>,
        local_peer_meta: PeerMetadata,
        remote_peer_metas: Vec<PeerMetadata>,
        peer_group_recipient: Recipient<ReceivePeerGroup>,
        peer_group_size: usize,
        peer_group_limit: usize,
        delta: Duration,
    ) -> Self {
        PeerBootstrapper {
            upgrader,
            local_peer_meta,
            remote_peer_metas,
            peer_group_recipient,
            peer_group_size,
            peer_group_limit,
            current_peer_group: vec![],
            delta,
            sent_peer_groups: Arc::new(AtomicUsize::new(0)),
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
        info!("received execute");
        // The peer handler uses this actor and sends `ReceivePeer` to it once a peer has been handled.
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
pub struct ReceivePeerGroup {
    pub group: Vec<PeerMetadata>,
}

impl Handler<ReceivePeer> for PeerBootstrapper {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: ReceivePeer, ctx: &mut Context<Self>) -> Self::Result {
        info!("received peer");
        // TODO: check the peer metadata more thoroughly
        if self.current_peer_group.len() >= self.peer_group_size {
            let group = self.current_peer_group.clone();
            self.current_peer_group = vec![];
            let peer_group_limit = self.peer_group_limit.clone();
            let peer_group_recipient = self.peer_group_recipient.clone();
            let sent_peer_groups = self.sent_peer_groups.clone();
            let bootstrapped = self.bootstrapped.clone();
            Box::pin(async move {
                peer_group_recipient.send(ReceivePeerGroup { group }).await;
                let n_sent_peer_groups = sent_peer_groups.load(Ordering::Relaxed);
                sent_peer_groups.store(n_sent_peer_groups + 1, Ordering::Relaxed);
                if n_sent_peer_groups + 1 >= peer_group_limit {
                    bootstrapped.store(true, Ordering::Relaxed);
                }
            })
        } else {
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

impl ResponseHandler for PeerHandler {
    fn handle_response(&self, response: Response) -> Pin<Box<dyn Future<Output = Result<()>>>> {
        info!("received peer bootstrap response");
        let recipient = self.recipient.clone();
        match response {
            Response::VersionAck(version_ack) => Box::pin(async move {
                if version_ack.version == version::CURRENT_VERSION {
                    if version_ack.peer_list.len() > MAX_PEER_LIST {
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

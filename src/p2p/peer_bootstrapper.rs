use crate::colored::Colorize;
use crate::protocol::{Request, Response};
use crate::version::Version;
use crate::{Error, Result};

use super::linear_backoff::Execute;
use super::peer_meta::PeerMetadata;
use super::sender::{multicast, Sender};

use crate::p2p::connection::ResponseHandler;
use crate::tls::upgrader::Upgrader;

use actix::{Actor, Handler, Recipient, ResponseFuture};
use actix::{ActorFutureExt, ResponseActFuture, WrapFuture};
use actix::{AsyncContext, Context};

use futures::Future;

use tokio::time::{timeout, Duration};

use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tracing::{debug, error, info, warn};

pub struct PeerBootstrapper {
    upgrader: Arc<dyn Upgrader>,
    local_peer_meta: PeerMetadata,
    remote_peer_metas: Vec<PeerMetadata>,
    delta: Duration,
    n_required_responses: usize,
    bootstrapped: AtomicBool,
}

impl PeerBootstrapper {
    pub fn new(
        upgrader: Arc<dyn Upgrader>,
        local_peer_meta: PeerMetadata,
        remote_peer_metas: Vec<PeerMetadata>,
        delta: Duration,
        n_required_responses: usize,
    ) -> Self {
        PeerBootstrapper {
            upgrader,
            local_peer_meta,
            remote_peer_metas,
            delta,
            n_required_responses,
            bootstrapped: AtomicBool::new(false),
        }
    }
}

impl Actor for PeerBootstrapper {
    type Context = Context<Self>;

    fn stopped(&mut self, ctx: &mut Context<Self>) {
        info!("[bootstrapper] stopped");
    }
}

impl Handler<Execute> for PeerBootstrapper {
    type Result = ResponseActFuture<Self, bool>;

    fn handle(&mut self, msg: Execute, ctx: &mut Context<Self>) -> Self::Result {
        let self_recipient = ctx.address().recipient().clone();
        let peer_handler = PeerHandler::new(self_recipient);
        let sender_address = Sender::new(self.upgrader.clone(), peer_handler).start();
        let request = Request::Version(Version::new(self.local_peer_meta.clone()));
        let multicast_fut =
            multicast(sender_address, self.remote_peer_metas.clone(), request, self.delta.clone());
        let multicast_wrapped = actix::fut::wrap_future::<_, Self>(multicast_fut);
        let n_required_responses = self.n_required_responses.clone();
        Box::pin(multicast_wrapped.map(move |responses, _actor, ctx| {
            true
            // if responses.len() >= n_required_responses {
            //     // check version compatibility
            //     info!(
            //         "[{}] obtained bootstrap quorum {}",
            //         "peer_bootstrapper".green(),
            //         "✓".green()
            //     );
            //     true
            // } else {
            //     false
            // }
        }))
    }
}

impl Handler<ReceivePeer> for PeerBootstrapper {
    type Result = ();

    fn handle(&mut self, msg: ReceivePeer, ctx: &mut Context<Self>) -> Self::Result {}
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct ReceivePeer {
    pub peer: Response,
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
        Box::pin(async { Ok(()) })
    }
}

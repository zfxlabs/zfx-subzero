use crate::{Result, Error};
use crate::version::Version;
use crate::protocol::{Request, Response};

use super::peer_meta::PeerMetadata;
use super::sender::{Sender, multicast};
use super::backoff::Execute;

use crate::tls::upgrader::Upgrader;

use actix::{AsyncContext, Context};
use actix::{Actor, Handler, ResponseFuture, Recipient};
use actix::{ActorFutureExt, ResponseActFuture, WrapFuture};

use tokio::time::{timeout, Duration};

use std::sync::Arc;

use tracing::{debug, info, warn, error};

#[derive(Clone)]
pub struct Bootstrapper {
    upgrader: Arc<dyn Upgrader>,
    local_peer_meta: PeerMetadata,
    remote_peer_metas: Vec<PeerMetadata>,
    delta: Duration,
    n_required_responses: usize,
}

impl Bootstrapper {
    pub fn new(upgrader: Arc<dyn Upgrader>, local_peer_meta: PeerMetadata, remote_peer_metas: Vec<PeerMetadata>, delta: Duration, n_required_responses: usize) -> Self {
	Bootstrapper { upgrader, local_peer_meta, remote_peer_metas, delta, n_required_responses }
    }
}

impl Actor for Bootstrapper {
    type Context = Context<Self>;

    fn stopped(&mut self, ctx: &mut Context<Self>) {
	info!("[bootstrapper] stopped");
    }
}

impl Handler<Execute> for Bootstrapper {
    type Result = ResponseActFuture<Self, bool>;

    fn handle(&mut self, msg: Execute, ctx: &mut Context<Self>) -> Self::Result {
	let sender_address = Sender::new(self.upgrader.clone()).start();
	let request = Request::Version(Version {
	    id: self.local_peer_meta.id,
	    ip: self.local_peer_meta.ip,
	});
	let multicast_fut = multicast(sender_address, self.remote_peer_metas.clone(), self.delta.clone(), request);
	let multicast_wrapped = actix::fut::wrap_future::<_, Self>(multicast_fut);
	let n_required_responses = self.n_required_responses.clone();
	Box::pin(multicast_wrapped.map(move |responses, _actor, ctx| {
	    if responses.len() >= n_required_responses {
		true
	    } else {
		false
	    }
	}))
    }
}

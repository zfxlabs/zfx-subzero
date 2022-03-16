use crate::{Result, Error};

use super::peer_meta::PeerMetadata;

use super::connection::{Connection, ConnectionState};
use super::connector::{Connector, Connect};

use crate::tls::connection_stream::ConnectionStream;
use crate::tls::upgrader::Upgrader;
use crate::channel::Channel;
use crate::protocol::{Request, Response};

use actix::{Actor, Context, Handler, Addr, Recipient, ResponseFuture};

use tokio::time::{timeout, Duration};

use std::sync::Arc;
use std::net::SocketAddr;

use tracing::{info, error};

pub struct Sender {
    upgrader: Arc<dyn Upgrader>,
}

impl Sender {
    pub fn new(upgrader: Arc<dyn Upgrader>) -> Self {
	Sender { upgrader }
    }
}

impl Actor for Sender {
    type Context = Context<Self>;

    fn stopped(&mut self, ctx: &mut Context<Self>) {
	info!("[sender] stopped");
    }
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<Option<Response>>")]
pub struct Send {
    pub peer_meta: PeerMetadata,
    pub delta: Duration,
    pub request: Request,
}

impl Send {
    pub fn new(peer_meta: PeerMetadata, delta: Duration, request: Request) -> Self {
	Send { peer_meta, delta, request }
    }
}

impl Handler<Send> for Sender {
    type Result = ResponseFuture<Result<Option<Response>>>;

    fn handle(&mut self, msg: Send, ctx: &mut Context<Self>) -> Self::Result {
	let upgrader = self.upgrader.clone();
	let execution = async move {
	    let connector_address = Connector::new(upgrader).start();
	    let connect = Connect::new(msg.peer_meta, msg.delta);
	    match connector_address.send(connect).await.unwrap() {
		Ok(Some(connection_stream)) => {
		    let mut channel: Channel<Request, Response> =
			Channel::wrap(connection_stream).unwrap();
		    let (mut sender, mut receiver) = channel.split();
		    let () = sender.send(msg.request).await.unwrap();
		    match timeout(msg.delta, receiver.recv()).await {
			Ok(result) =>
			    result.map_err(|_| Error::Timeout),
			Err(_) =>
			    Err(Error::Timeout),
		    }
		},
		Ok(None) =>
		    Err(Error::EmptyConnection),
		Err(err) =>
		    Err(err),
	    }
	};
	Box::pin(execution)
    }
}

pub async fn send(sender: Addr<Sender>, peer_meta: PeerMetadata, delta: Duration, request: Request) -> Result<Response> {
    let send = Send::new(peer_meta, delta, request);
    match sender.send(send).await.map_err(|_| Error::ActixMailboxError)? {
	Ok(Some(response)) =>
	    Ok(response),
	Ok(None) =>
	    Err(Error::EmptyResponse),
	Err(err) =>
	    Err(err),
    }
}

pub async fn multicast(sender: Addr<Sender>, peer_metas: Vec<PeerMetadata>, delta: Duration, request: Request) -> Vec<Response> {
    let mut responses = vec![];
    for peer_meta in peer_metas.iter().cloned() {
	match send(sender.clone(), peer_meta, delta, request.clone()).await {
	    Ok(response) =>
		responses.push(response),
	    Err(err) =>
		error!("{:?}", err),
	}
    }
    responses
}

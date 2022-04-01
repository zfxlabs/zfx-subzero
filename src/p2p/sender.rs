use crate::{Error, Result};

use super::peer_meta::PeerMetadata;

use super::connection::{self, ConnectionHandler, ResponseHandler, Upgraded};
use super::connection_factory::{Connect, ConnectionFactory};

use crate::channel::Channel;
use crate::protocol::{Request, Response};
use crate::tls::connection_stream::ConnectionStream;
use crate::tls::upgrader::Upgrader;

use actix::{Actor, Addr, Context, Handler, Recipient, ResponseFuture};

use tokio::time::{timeout, Duration};

use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use futures::Future;

use tracing::{error, info};

pub struct Sender {
    upgrader: Arc<dyn Upgrader>,
    response_handler: Arc<dyn ResponseHandler>,
}

impl Sender {
    pub fn new(upgrader: Arc<dyn Upgrader>, response_handler: Arc<dyn ResponseHandler>) -> Self {
        Sender { upgrader, response_handler }
    }
}

impl Actor for Sender {
    type Context = Context<Self>;

    fn stopped(&mut self, ctx: &mut Context<Self>) {
        info!("[sender] stopped");
    }
}

#[derive(Clone, Message)]
#[rtype(result = "Result<()>")]
pub struct Send {
    pub peer_meta: PeerMetadata,
    pub request: Request,
    pub delta: Duration,
}

impl Send {
    pub fn new(peer_meta: PeerMetadata, request: Request, delta: Duration) -> Self {
        Send { peer_meta, request, delta }
    }
}

impl Handler<Send> for Sender {
    type Result = ResponseFuture<Result<()>>;

    fn handle(&mut self, msg: Send, ctx: &mut Context<Self>) -> Self::Result {
        let upgrader = self.upgrader.clone();
        let response_handler = self.response_handler.clone();
        let execution = async move {
            let factory_address =
                ConnectionFactory::new(upgrader, msg.request, response_handler).start();
            let connect = Connect::new(msg.peer_meta);
            factory_address.send(connect).await.unwrap()
        };
        Box::pin(execution)
    }
}

pub async fn send(
    sender: Addr<Sender>,
    peer_meta: PeerMetadata,
    request: Request,
    delta: Duration,
) -> Result<()> {
    let send = Send::new(peer_meta, request, delta);
    sender.send(send).await.map_err(|_| Error::ActixMailboxError)?
}

pub async fn multicast(
    sender: Addr<Sender>,
    peer_metas: Vec<PeerMetadata>,
    request: Request,
    delta: Duration,
) -> Result<()> {
    for peer_meta in peer_metas.iter().cloned() {
        send(sender.clone(), peer_meta, request.clone(), delta).await?
    }
    Ok(())
}

//! The network sender is responsible for sending external requests from `p2p` actors to other peers
//! on the network. A `Send` sends to one peer and a `Multicast` sends to many peers.

use crate::{Error, Result};

use super::prelude::*;

use super::connection::{self, Upgraded};
use super::connection_factory::{Connect, ConnectionFactory};
use super::connection_handler::ConnectionHandler;
use super::response_handler::ResponseHandler;

use crate::channel::Channel;

use crate::protocol::{Request, Response};

use std::collections::HashSet;
use std::net::SocketAddr;

pub struct Sender<Rsp: Response> {
    /// Upgrades the connection to TLS or plain TCP according to configuration.
    upgrader: Arc<dyn Upgrader>,
    /// Handles a response from a peer after processing a `Send` request.
    response_handler: Arc<dyn ResponseHandler<Rsp>>,
}

impl<Rsp: Response> Sender<Rsp> {
    pub fn new(
        upgrader: Arc<dyn Upgrader>,
        response_handler: Arc<dyn ResponseHandler<Rsp>>,
    ) -> Self {
        Sender { upgrader, response_handler }
    }
}

impl<Rsp: Response> Actor for Sender<Rsp> {
    type Context = Context<Self>;
}

#[derive(Clone, Message)]
#[rtype(result = "Result<()>")]
pub struct Send<Req: Request> {
    pub peer_meta: PeerMetadata,
    pub request: Req,
    pub delta: Duration,
}

impl<Req: Request> Send<Req> {
    pub fn new(peer_meta: PeerMetadata, request: Req, delta: Duration) -> Self {
        Send { peer_meta, request, delta }
    }
}

impl<Req: Request, Rsp: Response> Handler<Send<Req>> for Sender<Rsp> {
    type Result = ResponseFuture<Result<()>>;

    fn handle(&mut self, msg: Send<Req>, ctx: &mut Context<Self>) -> Self::Result {
        let upgrader = self.upgrader.clone();
        let response_handler = self.response_handler.clone();
        let execution = async move {
            let factory_address =
                ConnectionFactory::new(upgrader, msg.request, response_handler).start();
            let connect = Connect::new(msg.peer_meta);
            // info!("sending {:?} to connection factory", connect.clone());
            factory_address.send(connect).await.unwrap()
        };
        Box::pin(execution)
    }
}

pub async fn send<Req: Request, Rsp: Response>(
    sender: Addr<Sender<Rsp>>,
    peer_meta: PeerMetadata,
    request: Req,
    delta: Duration,
) -> Result<()> {
    let send = Send::new(peer_meta, request, delta);
    sender.send(send).await.map_err(|_| Error::ActixMailboxError)?
}

pub async fn multicast<Req: Request, Rsp: Response>(
    sender: Addr<Sender<Rsp>>,
    peer_metas: HashSet<PeerMetadata>,
    request: Req,
    delta: Duration,
) -> Result<()> {
    for peer_meta in peer_metas.iter().cloned() {
        send(sender.clone(), peer_meta, request.clone(), delta).await?
    }
    Ok(())
}

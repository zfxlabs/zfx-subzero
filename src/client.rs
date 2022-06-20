use crate::channel::Channel;
use crate::protocol::{Request, Response};
use crate::tls::upgrader::Upgrader;
use crate::zfx_id::Id;
use crate::{Error, Result};

use tracing::{debug, error, warn};

use tokio::net::TcpStream;

use actix::{Actor, Context, Handler, ResponseFuture};
use futures::FutureExt;
use std::net::SocketAddr;
use std::sync::Arc;

/// Client is responsible for making requests to one or many nodes in the network.
/// Its main handler is [ClientRequest] which accepts [ClientRequest::Oneshot] or [ClientRequest::Fanout]
pub struct Client {
    /// For upgrading a [TcpStream] to a [ConnectionStream](crate::tls::connection_stream::ConnectionStream)
    upgrader: Arc<dyn Upgrader>,
}

impl Client {
    /// Creates a new client with an upgrader for the channel
    /// (ex. [TCP](crate::tls::upgrader::TcpUpgrader) or [TLS](crate::tls::upgrader::TlsClientUpgrader))
    pub fn new(upgrader: Arc<dyn Upgrader>) -> Client {
        Client { upgrader }
    }
}

impl Actor for Client {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
        debug!("started client");
    }
}

/// This structure is intended for sending a [Request](crate::protocol::Request) to one or many nodes, passed through the [Client].
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "ClientResponse")]
pub enum ClientRequest {
    /// For single request to a node and wait for a response
    Oneshot { id: Id, ip: SocketAddr, request: Request },
    /// For single request to many nodes and wait for all responses
    Fanout { peers: Vec<(Id, SocketAddr)>, request: Request },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientResponse {
    Oneshot(Option<Response>),
    Fanout(Vec<Response>),
}

impl Handler<ClientRequest> for Client {
    type Result = ResponseFuture<ClientResponse>;

    fn handle(&mut self, msg: ClientRequest, _ctx: &mut Context<Self>) -> Self::Result {
        let upgrader = self.upgrader.clone();
        match msg {
            ClientRequest::Oneshot { id, ip, request } => Box::pin(async move {
                let response = oneshot(id.clone(), ip.clone(), request.clone(), upgrader).await;
                ClientResponse::Oneshot(err_to_none(response))
            }),
            ClientRequest::Fanout { peers, request } => Box::pin(async move {
                ClientResponse::Fanout(fanout(peers.clone(), request.clone(), upgrader).await)
            }),
        }
    }
}

// TODO this shouldn't be `pub` but `client_test` is using it

/// Send a request to a node with Id and IP-address and returns a response.
/// * `id` - Id of a node, usually known at startup
/// * `ip` - IP-address of a node, corresponding to the node Id
/// * `request` - Request to send
/// * `upgrader` - an [upgrader](crate::tls::upgrader::Upgrader) for the node (ex. TCP or TLS)
///
/// This function is mainly used by the [Client] actor inside its handler for requests.
pub async fn oneshot(
    id: Id,
    ip: SocketAddr,
    request: Request,
    upgrader: Arc<dyn Upgrader>,
) -> Result<Option<Response>> {
    let socket = TcpStream::connect(&ip).await.map_err(Error::IO)?;
    let connection = upgrader.upgrade(socket).await?;
    if connection.is_tls()
        && id != connection.get_id().map_err(|_| Error::UnexpectedPeerConnected)?
    {
        warn!("connected peer id doesn't match expected id");
        return Err(Error::UnexpectedPeerConnected);
    }
    let mut channel: Channel<Request, Response> = Channel::wrap(connection)?;
    let (mut sender, mut receiver) = channel.split();
    let () = sender.send(request).await?;
    let response = receiver.recv().await?;
    Ok(response)
}

/// To be used in the integration tests (TCP-only)
#[cfg(test)]
pub async fn oneshot_tcp(ip: SocketAddr, request: Request) -> Result<Option<Response>> {
    oneshot(Id::zero(), ip, request, crate::tls::upgrader::TcpUpgrader::new()).await
}

/// Send a request to many nodes with Id and IP-addresses and collects responses.
/// * `id` - Id of a node, usually known at startup.
/// * `ip` - IP-address of a node, corresponding to the node Id.
/// * `request` - Request to send
/// * `upgrader` - an upgrader for the node (ex. TCP or TLS) [see here](crate::tls::upgrader::Upgrader) for more details.
///
/// This function is mainly used by the [Client] actor inside its handler for requests.
async fn fanout(
    peers: Vec<(Id, SocketAddr)>,
    request: Request,
    upgrader: Arc<dyn Upgrader>,
) -> Vec<Response> {
    let mut client_futs = vec![];
    // fanout oneshot requests to the ips designated in `ips` and collect the client
    // futures.
    for (id, ip) in peers.iter().cloned() {
        let request = request.clone();
        let upgrader = upgrader.clone();
        let client_fut =
            tokio::spawn(
                async move { err_to_none(oneshot(id, ip, request.clone(), upgrader).await) },
            );
        client_futs.push(client_fut)
    }
    // join the futures and collect the responses
    futures::future::join_all(client_futs)
        .map(|results| {
            let mut responses = vec![];
            for r in results.iter() {
                match r {
                    Ok(inner) => match inner {
                        Some(response) => responses.push(response.clone()),
                        None => (),
                    },
                    // NOTE: The error here is logged and `None` is returned
                    Err(_) => error!("error: joining client futures"),
                }
            }
            responses
        })
        .await
}

/// Helper function to simplify the return value of the `oneshot` function
#[inline]
fn err_to_none<T>(x: Result<Option<T>>) -> Option<T> {
    match x {
        Ok(result) => result,
        // NOTE: The error here is logged and `None` is returned
        Err(err) => match err {
            Error::ChannelError(s) => {
                debug!("{}", s);
                None
            }
            err => {
                debug!("{:?}", err);
                None
            }
        },
    }
}

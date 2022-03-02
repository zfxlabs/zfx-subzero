use crate::channel::Channel;
use crate::protocol::{Request, Response};
use crate::tls::upgrader::Upgrader;
use crate::zfx_id::Id;
use crate::{Error, Result};

use tracing::{debug, error};

use tokio::net::TcpStream;

use actix::{Actor, Context, Handler, ResponseFuture};
use futures::FutureExt;
use std::net::SocketAddr;
use std::sync::Arc;

pub struct Client {
    upgrader: Arc<dyn Upgrader>,
}

impl Client {
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

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "ClientResponse")]
pub enum ClientRequest {
    /// Sends a single request and waits for a response.
    Oneshot {
        id: Id,
        ip: SocketAddr,
        request: Request,
    },
    Fanout {
        peers: Vec<(Id, SocketAddr)>,
        request: Request,
    },
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
pub async fn oneshot(
    id: Id,
    ip: SocketAddr,
    request: Request,
    upgrader: Arc<dyn Upgrader>,
) -> Result<Option<Response>> {
    let socket = TcpStream::connect(&ip).await.map_err(Error::IO)?;
    let connection = upgrader.upgrade(socket).await?;
    let mut channel: Channel<Request, Response> = Channel::wrap(connection)?;
    let (mut sender, mut receiver) = channel.split();
    // send a message to a peer
    let () = sender.send(request).await?;
    // await a response
    let response = receiver.recv().await?;
    // debug!("<-- {:?}", response.clone());
    // ... close the connection by dropping the sender / receiver
    // return the response
    Ok(response)
}

// TODO will this work??
#[cfg(test)]
pub async fn oneshot_tcp(ip: SocketAddr, request: Request) -> Result<Option<Response>> {
    oneshot(Id::from_ip(&ip), ip, request, TcpUpgrader::new()).await
}

/// A gentle fanout function which sends requests to peers and collects responses.
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

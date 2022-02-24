use crate::channel::Channel;
use crate::protocol::{Request, Response};
use crate::tls::upgrader::{TcpUpgrader, Upgrader};
use crate::{Error, Result};
use tracing::{debug, error};

use tokio::net::TcpStream;

use actix::{Actor, Context, Handler, ResponseFuture};
use futures::FutureExt;
use std::net::SocketAddr;
use std::sync::Arc;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref FIXME_UPGRADER: Arc<dyn Upgrader> = TcpUpgrader::new();
}

pub struct Client {
    upgrader: Arc<dyn Upgrader>,
}

impl Client {
    pub fn new() -> Client {
        Client { upgrader: TcpUpgrader::new() }
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
        ip: SocketAddr,
        request: Request,
    },
    Fanout {
        ips: Vec<SocketAddr>,
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
            ClientRequest::Oneshot { ip, request } => Box::pin(async move {
                let response = oneshot(ip.clone(), request.clone(), upgrader).await;
                ClientResponse::Oneshot(err_to_none(response))
            }),
            ClientRequest::Fanout { ips, request } => Box::pin(async move {
                ClientResponse::Fanout(fanout(ips.clone(), request.clone(), upgrader).await)
            }),
        }
    }
}

pub async fn oneshot(
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

/// A gentle fanout function which sends requests to peers and collects responses.
pub async fn fanout(
    ips: Vec<SocketAddr>,
    request: Request,
    upgrader: Arc<dyn Upgrader>,
) -> Vec<Response> {
    let mut client_futs = vec![];
    // fanout oneshot requests to the ips designated in `ips` and collect the client
    // futures.
    for ip in ips.iter().cloned() {
        let request = request.clone();
        let upgrader = upgrader.clone();
        let client_fut =
            tokio::spawn(async move { err_to_none(oneshot(ip, request.clone(), upgrader).await) });
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

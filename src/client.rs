use crate::channel::Channel;
use crate::protocol::{Request, Response};
use crate::{Error, Result};
use tracing::{debug, error};

use actix::{Actor, Context, Handler, ResponseFuture};
use futures::FutureExt;
use std::net::SocketAddr;

pub struct Client;

impl Client {
    pub fn new() -> Client {
        Client {}
    }
}

impl Actor for Client {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
        debug!("started client");
    }
}

/// Sends a single request and waits for a response.
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Result<Option<Response>>")]
pub struct Oneshot {
    pub ip: SocketAddr,
    pub request: Request,
}

impl Handler<Oneshot> for Client {
    type Result = ResponseFuture<Result<Option<Response>>>;

    fn handle(&mut self, msg: Oneshot, _ctx: &mut Context<Self>) -> Self::Result {
        Box::pin(async move { oneshot(msg.ip.clone(), msg.request.clone()).await })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Vec<Response>")]
pub struct Fanout {
    pub ips: Vec<SocketAddr>,
    pub request: Request,
}

impl Handler<Fanout> for Client {
    type Result = ResponseFuture<Vec<Response>>;

    fn handle(&mut self, msg: Fanout, _ctx: &mut Context<Self>) -> Self::Result {
        Box::pin(async move { fanout(msg.ips.clone(), msg.request.clone()).await })
    }
}

pub async fn oneshot(ip: SocketAddr, request: Request) -> Result<Option<Response>> {
    let mut channel: Channel<Request, Response> = Channel::connect(&ip).await?;
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
pub async fn fanout(ips: Vec<SocketAddr>, request: Request) -> Vec<Response> {
    let mut client_futs = vec![];
    // fanout oneshot requests to the ips designated in `ips` and collect the client
    // futures.
    for ip in ips.iter().cloned() {
        let request = request.clone();
        let client_fut = tokio::spawn(async move {
            match oneshot(ip, request.clone()).await {
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
        });
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

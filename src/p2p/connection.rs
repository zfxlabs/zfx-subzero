use crate::{Error, Result};

use crate::channel::Channel;
use crate::zfx_id::Id;

use super::peer_meta::PeerMetadata;

use crate::protocol::{Request, Response};

use crate::tls::connection_stream::ConnectionStream;
use crate::tls::upgrader::Upgrader;

use actix::{Actor, Handler, Recipient, ResponseFuture};
use actix::{ActorFutureExt, ResponseActFuture, WrapFuture};
use actix::{AsyncContext, Context};

use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

use futures::Future;

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use tracing::{info, warn};

pub struct Connection<S> {
    pub upgrader: Arc<dyn Upgrader>,
    pub state: S,
}

pub struct Start;
pub struct Connected {
    pub peer_meta: PeerMetadata,
    pub tcp_stream: TcpStream,
}
pub struct Upgraded {
    pub peer_meta: PeerMetadata,
    pub connection_stream: ConnectionStream,
}

impl Connection<Start> {
    pub fn new(upgrader: Arc<dyn Upgrader>) -> Self {
        Connection { upgrader, state: Start }
    }
}

impl Connection<Connected> {
    pub async fn upgrade(self, upgrader: Arc<dyn Upgrader>) -> Result<ConnectionStream> {
        let stream = upgrader.upgrade(self.state.tcp_stream).await.map_err(Error::IO)?;
        if stream.is_tls() {
            if self.state.peer_meta.id != stream.get_id().map_err(|_| Error::UnexpectedPeer)? {
                return Err(Error::UnexpectedPeer);
            }
        }
        Ok(stream)
    }
}

impl<T: 'static + Unpin> Actor for Connection<T> {
    type Context = Context<Self>;
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<Connection<Connected>>")]
pub struct Connect {
    pub peer_meta: PeerMetadata,
}

impl Connect {
    pub fn new(peer_meta: PeerMetadata) -> Self {
        Connect { peer_meta }
    }
}

impl Handler<Connect> for Connection<Start> {
    type Result = ResponseActFuture<Self, Result<Connection<Connected>>>;

    fn handle(&mut self, msg: Connect, ctx: &mut Context<Self>) -> Self::Result {
        let peer_meta = msg.peer_meta.clone();
        let upgrader = self.upgrader.clone();
        let fut = TcpStream::connect(msg.peer_meta.ip);
        let fut_wrapped = actix::fut::wrap_future::<_, Self>(fut);
        Box::pin(fut_wrapped.map(move |rsp, actor, ctx| match rsp {
            Ok(tcp_stream) => {
                Ok(Connection { upgrader, state: Connected { peer_meta, tcp_stream } })
            }
            Err(err) => Err(err.into()),
        }))
    }
}

#[derive(Clone)]
pub struct ConnectionHandler {
    request: Request,
    send_timeout: Duration,
    response_handler: Arc<dyn ResponseHandler>,
}

impl ConnectionHandler {
    pub fn new(
        request: Request,
        send_timeout: Duration,
        response_handler: Arc<dyn ResponseHandler>,
    ) -> Self {
        ConnectionHandler { request, send_timeout, response_handler }
    }

    pub fn handle_connection(
        &self,
        connection: Connection<Upgraded>,
    ) -> Pin<Box<dyn Future<Output = Result<()>>>> {
        let request = self.request.clone();
        let response_handler = self.response_handler.clone();
        let send_timeout = self.send_timeout.clone();
        let mut channel: Channel<Request, Response> =
            Channel::wrap(connection.state.connection_stream).unwrap();
        let (mut sender, mut receiver) = channel.split();
        Box::pin(async move {
            let () = sender.send(request).await.unwrap();
            match timeout(send_timeout, receiver.recv()).await {
                Ok(res) => match res {
                    Ok(Some(response)) => response_handler.handle_response(response).await,
                    Ok(None) => Err(Error::EmptyResponse),
                    Err(err) => Err(err.into()),
                },
                Err(_) => Err(Error::Timeout),
            }
        })
    }
}

pub trait ResponseHandler {
    fn handle_response(&self, response: Response) -> Pin<Box<dyn Future<Output = Result<()>>>>;
}

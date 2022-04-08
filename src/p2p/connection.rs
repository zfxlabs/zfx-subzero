use super::prelude::*;

use tokio::net::TcpStream;

use std::collections::HashMap;

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
        info!("upgrading connection ...");
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
        info!("handling connect ...");
        let peer_meta = msg.peer_meta.clone();
        let upgrader = self.upgrader.clone();
        let fut = TcpStream::connect(msg.peer_meta.ip);
        let fut_wrapped = actix::fut::wrap_future::<_, Self>(fut);
        Box::pin(fut_wrapped.map(move |rsp, actor, ctx| match rsp {
            Ok(tcp_stream) => {
                Ok(Connection { upgrader, state: Connected { peer_meta, tcp_stream } })
            }
            Err(err) => {
                error!("[connection] {:?}", err);
                Err(err.into())
            }
        }))
    }
}

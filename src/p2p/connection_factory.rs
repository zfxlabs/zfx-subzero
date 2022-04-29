use super::prelude::*;

use tokio::net::TcpStream;
use tokio::time::Duration;

use std::collections::HashMap;

use super::connection::{self, Connected, Connection, Upgraded};
use super::connection_handler::ConnectionHandler;
use super::response_handler::ResponseHandler;

pub struct ConnectionFactory {
    pub upgrader: Arc<dyn Upgrader>,
    pub handler: ConnectionHandler,
}

impl ConnectionFactory {
    pub fn new(
        upgrader: Arc<dyn Upgrader>,
        request: Request,
        response_handler: Arc<dyn ResponseHandler>,
    ) -> Self {
        let delta = Duration::from_millis(1000u64);
        ConnectionFactory {
            upgrader,
            handler: ConnectionHandler::new(request, delta, response_handler),
        }
    }
}

impl Actor for ConnectionFactory {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "Result<()>")]
struct HandleConnection {
    pub connection: Connection<Upgraded>,
}

impl Handler<HandleConnection> for ConnectionFactory {
    type Result = ResponseActFuture<Self, Result<()>>;

    fn handle(&mut self, msg: HandleConnection, ctx: &mut Context<Self>) -> Self::Result {
        let handler = self.handler.clone();
        let fut = handler.handle_connection(msg.connection);
        let fut_wrapped = actix::fut::wrap_future::<_, Self>(fut);
        Box::pin(fut_wrapped.map(move |res, _act, _ctx| res))
    }
}

#[derive(Message)]
#[rtype(result = "Result<()>")]
struct UpgradeConnection {
    pub connection: Connection<Connected>,
}

impl Handler<UpgradeConnection> for ConnectionFactory {
    type Result = ResponseActFuture<Self, Result<()>>;

    fn handle(&mut self, msg: UpgradeConnection, ctx: &mut Context<Self>) -> Self::Result {
        let upgrader = self.upgrader.clone();
        let peer_meta = msg.connection.state.peer_meta.clone();
        let fut = msg.connection.upgrade(self.upgrader.clone());
        let fut_wrapped = actix::fut::wrap_future::<_, Self>(fut);
        Box::pin(fut_wrapped.map(move |res, _act, ctx| match res {
            Ok(connection_stream) => {
                let connection =
                    Connection { upgrader, state: Upgraded { peer_meta, connection_stream } };
                ctx.notify(HandleConnection { connection });
                Ok(())
            }
            Err(err) => Err(err),
        }))
    }
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<()>")]
pub struct Connect {
    pub peer_meta: PeerMetadata,
}

impl Connect {
    pub fn new(peer_meta: PeerMetadata) -> Self {
        Connect { peer_meta }
    }
}

impl Handler<Connect> for ConnectionFactory {
    type Result = ResponseActFuture<Self, Result<()>>;

    fn handle(&mut self, msg: Connect, ctx: &mut Context<Self>) -> Self::Result {
        let upgrader = self.upgrader.clone();
        let connection_address = Connection::new(self.upgrader.clone()).start();
        let fut = connection_address.send(connection::Connect { peer_meta: msg.peer_meta.clone() });
        let fut_wrapped = actix::fut::wrap_future::<_, Self>(fut);
        Box::pin(fut_wrapped.map(move |rsp, _act, ctx| match rsp {
            Ok(res) => match res {
                Ok(connected) => {
                    ctx.notify(UpgradeConnection { connection: connected });
                    Ok(())
                }
                Err(err) => Err(err),
            },
            Err(err) => Err(err.into()),
        }))
    }
}

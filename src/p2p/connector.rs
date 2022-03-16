use crate::{Result, Error};

use super::peer_meta::PeerMetadata;

use super::connection::Connection;

use crate::tls::connection_stream::ConnectionStream;
use crate::tls::upgrader::Upgrader;

use actix::{Actor, Context, Handler, Recipient, ResponseFuture};

use tokio::time::{timeout, Duration};

use std::sync::Arc;
use std::net::SocketAddr;

pub struct Connector {
    upgrader: Arc<dyn Upgrader>,
}

impl Connector {
    pub fn new(upgrader: Arc<dyn Upgrader>) -> Self {
	Connector { upgrader }
    }
}

impl Actor for Connector {
    type Context = Context<Self>;
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<Option<ConnectionStream>>")]
pub struct Connect {
    pub peer_meta: PeerMetadata,
    pub delta: Duration,
}

impl Connect {
    pub fn new(peer_meta: PeerMetadata, delta: Duration) -> Self {
	Connect { peer_meta, delta }
    }
}

impl Handler<Connect> for Connector {
    type Result = ResponseFuture<Result<Option<ConnectionStream>>>;

    fn handle(&mut self, msg: Connect, ctx: &mut Context<Self>) -> Self::Result {
	let upgrader = self.upgrader.clone();
	Box::pin(async move {
	    let mut connection = Connection::new(msg.peer_meta);
	    if let Ok(result) = timeout(msg.delta, connection.connect(upgrader)).await {
		if let Err(err) = result {
		    Err(err)
		} else {
		    match connection.upgrade().await {
			Ok(connection_stream) => 
			    Ok(Some(connection_stream)),
			Err(err) =>
			    Ok(None),
		    }
		}
	    } else {
		Err(Error::Timeout)
	    }
	})
    }
}

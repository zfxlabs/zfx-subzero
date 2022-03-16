use crate::{Result, Error};

use super::peer_meta::PeerMetadata;

use crate::tls::connection_stream::ConnectionStream;
use crate::tls::upgrader::Upgrader;

use tokio::net::TcpStream;

use std::sync::Arc;

use tracing::{info, warn};

// struct StateMachine<S> { state: S }
// struct StateInit;
// struct StateConnected {
//   tcp_stream: TcpStream,
//   upgrader: Arc<dyn Upgrader>,
//}

pub enum ConnectionState {
    Init,
    Connected((TcpStream, Arc<dyn Upgrader>)),
    Upgraded,
}

pub struct Connection {
    pub peer_meta: PeerMetadata,
    pub state: ConnectionState,
}

impl Connection {
    pub fn new(peer_meta: PeerMetadata) -> Connection {
	Connection { peer_meta, state: ConnectionState::Init }
    }

    pub async fn connect(&mut self, upgrader: Arc<dyn Upgrader>) -> Result<()> {
	match self.state {
	    ConnectionState::Init => { 
		let tcp_stream = TcpStream::connect(&self.peer_meta.ip).await.map_err(Error::IO)?;
		self.state = ConnectionState::Connected((tcp_stream, upgrader));
		info!("[connection] connected.");
		Ok(())
	    },
	    _ => {
		warn!("[warning] called `connect` on a connection in an incompatible state");
		Err(Error::UnexpectedState)
	    },
	}
    }

    pub async fn upgrade(mut self: Self) -> Result<ConnectionStream> {
	match self.state {
	    ConnectionState::Connected((tcp_stream, upgrader)) => {
		let stream = upgrader.upgrade(tcp_stream).await?;
		if stream.is_tls() && self.peer_meta.id != stream.get_id().map_err(|_| Error::UnexpectedPeer)? {
		    warn!("");
		    return Err(Error::UnexpectedPeer);
		}
		self.state = ConnectionState::Upgraded;
		Ok(stream)
	    },
	    _ => {
		warn!("[warning] called `upgrade` on a connection in an incompatible state");
		Err(Error::UnexpectedState)
	    },
	}
    }
}

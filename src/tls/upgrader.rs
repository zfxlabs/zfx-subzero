use std::future::Future;
use std::{pin::Pin, sync::Arc};

use crate::tls::tls::{client_tls_config, server_tls_config, DUMMY_DOMAIN};
use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, TlsConnector};

use super::connection_stream::ConnectionStream;

pub trait Upgrader: Sync + Send {
    // === async fn upgrade(..) -> Result<ConnectionStream>;
    fn upgrade(&self, conn: TcpStream) -> UpgradeOutput;

    fn is_tls(&self) -> bool;
}

pub struct TcpUpgrader {}

pub struct TlsClientUpgrader {
    connector: TlsConnector,
}

pub struct TlsServerUpgrader {
    acceptor: TlsAcceptor,
}

/// Future type that can be safely held across `.await` boundaries
/// and is compatible with Tokio
type SafeFuture<Out> = Pin<Box<dyn Sync + Send + Future<Output = Out>>>;

type UpgradeOutput = SafeFuture<Result<ConnectionStream, std::io::Error>>;

impl TcpUpgrader {
    pub fn new() -> Arc<dyn Upgrader> {
        Arc::new(TcpUpgrader {})
    }
}
impl Upgrader for TcpUpgrader {
    fn upgrade(&self, conn: TcpStream) -> UpgradeOutput {
        let fut = async { Ok(ConnectionStream::Tcp(conn)) };
        Box::pin(fut)
    }

    fn is_tls(&self) -> bool {
        false
    }
}

impl TlsClientUpgrader {
    pub fn new(cert: &[u8], key: &[u8]) -> Arc<dyn Upgrader> {
        let config = client_tls_config(cert, key);
        let connector = TlsConnector::from(Arc::new(config));
        Arc::new(TlsClientUpgrader { connector })
    }
}

impl Upgrader for TlsClientUpgrader {
    fn upgrade(&self, c: TcpStream) -> UpgradeOutput {
        let connector = self.connector.clone();
        let fut = async move {
            match connector.connect(DUMMY_DOMAIN.clone(), c).await {
                Ok(tls_stream) => Ok(ConnectionStream::TlsClient(tls_stream)),
                Err(e) => Err(e),
            }
        };
        Box::pin(fut)
    }

    fn is_tls(&self) -> bool {
        true
    }
}

impl TlsServerUpgrader {
    pub fn new(cert: &[u8], key: &[u8]) -> Arc<dyn Upgrader> {
        let config = server_tls_config(cert, key);
        let acceptor = TlsAcceptor::from(Arc::new(config));
        Arc::new(TlsServerUpgrader { acceptor })
    }
}

impl Upgrader for TlsServerUpgrader {
    fn upgrade(&self, c: TcpStream) -> UpgradeOutput {
        let acc = self.acceptor.clone();
        let fut = async move {
            match acc.accept(c).await {
                Ok(tls_stream) => Ok(ConnectionStream::TlsServer(tls_stream)),
                Err(e) => Err(e),
            }
        };
        Box::pin(fut)
    }

    fn is_tls(&self) -> bool {
        true
    }
}

pub struct Upgraders {
    pub client: Arc<dyn Upgrader>,
    pub server: Arc<dyn Upgrader>,
}

pub fn tls_upgraders(certificate: &[u8], private_key: &[u8]) -> Upgraders {
    Upgraders {
        client: TlsClientUpgrader::new(certificate, private_key),
        server: TlsServerUpgrader::new(certificate, private_key),
    }
}

pub fn tcp_upgraders() -> Upgraders {
    Upgraders { client: TcpUpgrader::new(), server: TcpUpgrader::new() }
}

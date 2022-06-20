//! Upgrade a `TcpStream` to a [ConnectionStream]
//!
//! [Upgrader]s wrap the connection into the [ConnectionStream] type, which reperents a TCP and TLS
//! connections.
//! For the TLS case, it involves the TLS handshake and setting up encrypted communication,
//! while for TCP, it is practically a no-op.

use std::future::Future;
use std::{pin::Pin, sync::Arc};

use crate::tls::tls::{client_tls_config, server_tls_config, DUMMY_DOMAIN};
use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, TlsConnector};

use super::connection_stream::ConnectionStream;

/// `Upgrader` represents an `async` trait, for upgrading a `TcpStream` to a `ConnectionStream`.
///
/// The implementors differ radically in their behaviour:
/// For the TLS case, it involves the TLS handshake and setting up encrypted communication,
/// while for TCP, it is practically a no-op.
pub trait Upgrader: Sync + Send {
    /// `==  async fn upgrade(..) -> Result<ConnectionStream>`
    fn upgrade(&self, conn: TcpStream) -> UpgradeOutput;

    /// True if if the `Upgrader` upgrades to TLS, false for TCP
    fn is_tls(&self) -> bool;
}

/// Generic [Upgrader] for TCP
pub struct TcpUpgrader {}

/// TLS [Upgrader] for client connections
pub struct TlsClientUpgrader {
    connector: TlsConnector,
}

/// TLS [Upgrader] for server connections
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

/// A pair of client and server-side upgraders
pub struct Upgraders {
    pub client: Arc<dyn Upgrader>,
    pub server: Arc<dyn Upgrader>,
}

/// Return a pair of (client and server) upgraders for TLS
///
/// Takes the certificate and private key as parameters.
pub fn tls_upgraders(certificate: &[u8], private_key: &[u8]) -> Upgraders {
    Upgraders {
        client: TlsClientUpgrader::new(certificate, private_key),
        server: TlsServerUpgrader::new(certificate, private_key),
    }
}

/// Return a pair of (client and server) upgraders for TCP
///
/// The returned `Upgrader`s `update` method will simply wrap the connection into
/// a [ConnrrectionStream][super::connection_stream::ConnectionStream].
pub fn tcp_upgraders() -> Upgraders {
    Upgraders { client: TcpUpgrader::new(), server: TcpUpgrader::new() }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tls::certificate;
    use crate::zfx_id::Id;
    use std::net::SocketAddr;
    use std::net::ToSocketAddrs;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::{self, sync::oneshot};

    #[actix_rt::test]
    async fn smoke_test() {
        let _upgraders = tcp_upgraders();
        let (cert, key) = certificate::generate_node_cert().unwrap();
        let _upgraders = tls_upgraders(&cert, &key);
    }

    type Res = Result<(), String>;

    #[tokio::test(flavor = "multi_thread")]
    async fn handshakeand_id_test() {
        let (tx, rx) = oneshot::channel::<Res>();
        let server = tokio::spawn(tls_server(tx));
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let client = tokio::spawn(tls_client());

        let sleep = tokio::time::sleep(std::time::Duration::from_secs(2));
        tokio::pin!(sleep);

        tokio::select! {
            _ = &mut sleep => panic!("Timeout"),
            res = rx => {
                match res {
                    Err(e) => panic!("{}", e),
                    Ok(r) => if let Err(e) = r { panic!("{}", e) },
                }
            }
        }

        let (server_id1, client_id1) = server.await.unwrap();
        let (server_id2, client_id2) = client.await.unwrap();
        assert_eq!(server_id1, server_id2);
        assert_eq!(client_id1, client_id2);
    }

    async fn tls_server(tx: oneshot::Sender<Res>) -> (Id, Id) {
        let addr: SocketAddr = ("localhost", 9899).to_socket_addrs().unwrap().next().unwrap();

        let listener = TcpListener::bind(&addr).await.expect("couldnt bind to address");
        let (stream, c_addr) = listener.accept().await.expect("conn failed");
        println!("incoming TCP connection from {:}", &c_addr);

        let (cert, key) = certificate::generate_node_cert().unwrap();
        let server_id = Id::new(&cert);
        let upgraders = tls_upgraders(&cert, &key);
        let mut tls_stream = upgraders.server.upgrade(stream).await.unwrap();
        assert!(tls_stream.is_tls());
        let client_id = tls_stream.get_id().unwrap();
        let mut buf: Vec<u8> = vec![];
        match tls_stream.read_buf(&mut buf).await.unwrap() {
            // Clients sends `b"OK", two bytes
            2 => {
                println!("Read {:?}", String::from_utf8(buf));
                let _notanerror = tls_stream.shutdown().await;
                tx.send(Ok(())).unwrap();
            }
            _ => tx.send(Err(String::from("couldn't read from stream"))).unwrap(),
        }
        (server_id, client_id)
    }

    async fn tls_client() -> (Id, Id) {
        let addr: SocketAddr = ("localhost", 9899).to_socket_addrs().unwrap().next().unwrap();

        let stream = TcpStream::connect(&addr).await.expect("couldnt connect");
        println!("TCP connection to {:}", &addr);
        let (cert, key) = certificate::generate_node_cert().unwrap();
        let client_id = Id::new(&cert);
        let upgraders = tls_upgraders(&cert, &key);
        let mut tls_stream = upgraders.client.upgrade(stream).await.unwrap();
        assert!(tls_stream.is_tls());
        let server_id = tls_stream.get_id().unwrap();
        let _ = tls_stream.write_all(b"OK").await.unwrap();
        tls_stream.flush().await.expect("couldnt flush stream");

        (server_id, client_id)
    }
}

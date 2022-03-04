//! This module contains the code for configuring `tokio_rustls` for a peer-to-peer setting
//! Client side authenication is enforced, but the server doesn't need a certificate chain,
//! as both use single self-signed certificates.

use lazy_static::lazy_static;
use std::convert::TryFrom;
use std::{sync::Arc, time::SystemTime};
use tokio_rustls::rustls::{
    self, client::ServerCertVerifier, server::ClientCertVerifier, Certificate, ClientConfig,
    ServerConfig, ServerName,
};

lazy_static! {
    pub static ref DUMMY_DOMAIN: ServerName = ServerName::try_from("example.org").unwrap();
}

/// Client verification: enforce the presence and check a single certificates
struct ZfxClientCertVerifier;

impl ClientCertVerifier for ZfxClientCertVerifier {
    fn verify_client_cert(
        &self,
        _end_entity: &Certificate,
        _intermediates: &[Certificate],
        _now: SystemTime,
    ) -> Result<rustls::server::ClientCertVerified, rustls::Error> {
        Ok(rustls::server::ClientCertVerified::assertion())
    }
    fn client_auth_root_subjects(&self) -> Option<rustls::DistinguishedNames> {
        Some(vec![])
    }

    fn offer_client_auth(&self) -> bool {
        true
    }

    fn client_auth_mandatory(&self) -> Option<bool> {
        Some(true)
    }
}

/// Server verification: don't check certificate chain and domain name, just the presence of a certificate
struct ZfxServerCertVerifier;

impl ServerCertVerifier for ZfxServerCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &Certificate,
        _intermediates: &[Certificate],
        _server_name: &ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

/// Build a client-side configuration from the certificate and private key, usable in a peer-to-peer network
pub fn client_tls_config(raw_certificate: &[u8], raw_private_key: &[u8]) -> ClientConfig {
    let cert_vec = vec![rustls::Certificate(Vec::from(raw_certificate))];
    let pk = rustls::PrivateKey(Vec::from(raw_private_key));
    let verifier = Arc::new(ZfxServerCertVerifier);
    let mut config = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(rustls::RootCertStore::empty())
        .with_single_cert(cert_vec, pk)
        .unwrap();
    // `dangerous` here only means that we're using our own certificate verification
    let mut config = config.dangerous();
    config.set_certificate_verifier(verifier);
    config.cfg.clone()
}

/// Build a server-side configuration from the certificate and private key, usable in a peer-to-peer network
pub fn server_tls_config(raw_certificate: &[u8], raw_private_key: &[u8]) -> ServerConfig {
    let cert_vec = vec![rustls::Certificate(Vec::from(raw_certificate))];
    let pk = rustls::PrivateKey(Vec::from(raw_private_key));
    let verifier = Arc::new(ZfxClientCertVerifier);
    ServerConfig::builder()
        .with_safe_defaults()
        .with_client_cert_verifier(verifier)
        .with_single_cert(cert_vec, pk)
        .unwrap()
}

#[cfg(test)]
mod test {
    use super::*;
    use std::env::temp_dir;
    use std::net::ToSocketAddrs;
    use std::path::PathBuf;
    use std::{net::SocketAddr, sync::Arc};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::{self, sync::oneshot};
    use tokio_rustls::{TlsAcceptor, TlsConnector};

    fn generate_file_in_tmp_dir(name: &str, extension: &str) -> PathBuf {
        temp_dir().join(format!("{}.{}", name, extension))
    }
    pub fn cert_and_key(name: &str) -> (Vec<u8>, Vec<u8>) {
        let crt_f = generate_file_in_tmp_dir(name, ".crt");
        let key_f = generate_file_in_tmp_dir(name, "key");
        let (cert, key) = crate::tls::certificate::get_node_cert(&crt_f, &key_f).unwrap();
        (cert, key)
    }
    #[actix_rt::test]
    async fn test0() {
        let (cert, key) = cert_and_key("test0");
        let client_conf = client_tls_config(&cert, &key);
        let server_conf = server_tls_config(&cert, &key);
        let _tls_acc = TlsAcceptor::from(Arc::new(server_conf));
        let _tls_conn = TlsConnector::from(Arc::new(client_conf));
    }

    type Res = Result<(), String>;

    #[tokio::test(flavor = "multi_thread")]
    async fn handshake_test() {
        let (tx, rx) = oneshot::channel::<Res>();
        tokio::spawn(tls_server(tx));
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        tokio::spawn(tls_client());

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
    }

    async fn tls_server(tx: oneshot::Sender<Res>) {
        let (cert, key) = cert_and_key("server");
        let addr: SocketAddr = ("localhost", 9889).to_socket_addrs().unwrap().next().unwrap();

        let listener = TcpListener::bind(&addr).await.expect("couldnt bind to address");
        let (stream, c_addr) = listener.accept().await.expect("conn failed");
        println!("incoming TCP connection from {:}", &c_addr);
        let tls_conf = server_tls_config(&cert, &key);
        let acceptor = TlsAcceptor::from(Arc::new(tls_conf));
        let mut tls_stream = acceptor.accept(stream).await.unwrap();
        let mut buf: Vec<u8> = vec![]; // that could overflow
        match tls_stream.read_buf(&mut buf).await.unwrap() {
            2 => {
                println!("Read {:?}", String::from_utf8(buf));
                let _notanerror = tls_stream.shutdown().await;
                tx.send(Ok(())).unwrap();
            }
            _ => tx.send(Err(String::from("couldn't read from stream"))).unwrap(),
        }
    }

    async fn tls_client() {
        let (cert, key) = cert_and_key("server");
        let addr: SocketAddr = ("localhost", 9889).to_socket_addrs().unwrap().next().unwrap();

        let stream = TcpStream::connect(&addr).await.expect("couldnt connect");
        println!("TCP connection to {:}", &addr);
        let tls_conf = client_tls_config(&cert, &key);
        let connector = TlsConnector::from(Arc::new(tls_conf));
        let mut tls_stream = connector.connect(DUMMY_DOMAIN.clone(), stream).await.unwrap();
        let _ = tls_stream.write_all(b"OK").await.unwrap();
        tls_stream.flush().await.expect("couldnt flush stream");
    }
}

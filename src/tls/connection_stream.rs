use std::io;
use std::{net::SocketAddr, pin::Pin};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

use crate::zfx_id::Id;

/// A unified type for TCP and TLS streams for uniform handling of connections
/// As it implements Tokio's `AsyncWrite` and `AsyncRead` traits, it is usable in
#[derive(Debug)]
pub enum ConnectionStream {
    Tcp(TcpStream),
    TlsServer(tokio_rustls::server::TlsStream<TcpStream>),
    TlsClient(tokio_rustls::client::TlsStream<TcpStream>),
}

impl ConnectionStream {
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        match self {
            Self::Tcp(s) => s.local_addr(),
            Self::TlsServer(s) => s.get_ref().0.local_addr(),
            Self::TlsClient(s) => s.get_ref().0.local_addr(),
        }
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        match self {
            Self::Tcp(s) => s.peer_addr(),
            Self::TlsServer(s) => s.get_ref().0.peer_addr(),
            Self::TlsClient(s) => s.get_ref().0.peer_addr(),
        }
    }

    pub fn is_tls(&self) -> bool {
        match self {
            Self::Tcp(s) => false,
            Self::TlsServer(s) => true,
            Self::TlsClient(s) => true,
        }
    }

    /// Generate an `Id` from the connection.
    /// For TCP, it's the hash of the IP address
    /// For TLS it's the hash of the certificated presented
    pub fn get_id(&self) -> io::Result<Id> {
        match self {
            Self::Tcp(s) => Ok(Id::from_ip(&s.peer_addr()?)),
            Self::TlsServer(s) => id_from_server_connection(s),
            Self::TlsClient(s) => id_from_client_connection(s),
        }
    }
}

// Note that the functions `id_from_server_connection` and `id_from_client_connection`
// are _not_ identical, as the type of the `state variable differs

/// Hash the presented certificate to an `Id`
pub fn id_from_server_connection(
    connection: &tokio_rustls::server::TlsStream<TcpStream>,
) -> io::Result<Id> {
    let state = connection.get_ref().1;
    id_from_first_cert(state.peer_certificates())
}

/// Hash the presented certificate to an `Id`
pub fn id_from_client_connection(
    connection: &tokio_rustls::client::TlsStream<TcpStream>,
) -> io::Result<Id> {
    let state = connection.get_ref().1;
    id_from_first_cert(state.peer_certificates())
}

/// Generate the peer ID from the first (and only) certificate
fn id_from_first_cert(certs: Option<&[rustls::Certificate]>) -> io::Result<Id> {
    match certs {
        Some(certs) => {
            if certs.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "no certificates present in TLS state",
                ));
            }
            Ok(Id::new(&certs[0].0))
        }
        None => Err(io::Error::new(io::ErrorKind::Other, "no certificates present in TLS state")),
    }
}

// Inspired by:
// https://github.com/tokio-rs/tls/blob/794659740dcc399f79058c4eba325ffd97474c7b/tokio-rustls/src/lib.rs#L245
//
impl AsyncWrite for ConnectionStream {
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match self.get_mut() {
            ConnectionStream::Tcp(x) => Pin::new(x).poll_write(cx, buf),
            ConnectionStream::TlsClient(x) => Pin::new(x).poll_write(cx, buf),
            ConnectionStream::TlsServer(x) => Pin::new(x).poll_write(cx, buf),
        }
    }

    #[inline]
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            ConnectionStream::Tcp(x) => Pin::new(x).poll_flush(cx),
            ConnectionStream::TlsClient(x) => Pin::new(x).poll_flush(cx),
            ConnectionStream::TlsServer(x) => Pin::new(x).poll_flush(cx),
        }
    }

    #[inline]
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            ConnectionStream::Tcp(x) => Pin::new(x).poll_shutdown(cx),
            ConnectionStream::TlsClient(x) => Pin::new(x).poll_shutdown(cx),
            ConnectionStream::TlsServer(x) => Pin::new(x).poll_shutdown(cx),
        }
    }
}

impl AsyncRead for ConnectionStream {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            ConnectionStream::Tcp(x) => Pin::new(x).poll_read(cx, buf),
            ConnectionStream::TlsClient(x) => Pin::new(x).poll_read(cx, buf),
            ConnectionStream::TlsServer(x) => Pin::new(x).poll_read(cx, buf),
        }
    }
}

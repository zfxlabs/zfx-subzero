use std::{net::SocketAddr, pin::Pin};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

#[derive(Debug)]
pub enum ConnectionStream {
    Tcp(TcpStream),
    TlsServer(tokio_rustls::server::TlsStream<TcpStream>),
    TlsClient(tokio_rustls::client::TlsStream<TcpStream>),
}

impl ConnectionStream {
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        match self {
            Self::Tcp(s) => s.local_addr(),
            Self::TlsServer(s) => s.get_ref().0.local_addr(),
            Self::TlsClient(s) => s.get_ref().0.local_addr(),
        }
    }

    pub fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        match self {
            Self::Tcp(s) => s.peer_addr(),
            Self::TlsServer(s) => s.get_ref().0.peer_addr(),
            Self::TlsClient(s) => s.get_ref().0.peer_addr(),
        }
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

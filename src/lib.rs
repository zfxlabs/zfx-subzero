#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate actix_derive;
extern crate colored;

pub mod alpha;
pub mod cell;
pub mod channel;
pub mod client;
pub mod graph;
pub mod hail;
pub mod ice;
pub mod integration_test;
pub mod porter;
pub mod protocol;
pub mod server;
pub mod sleet;
pub mod storage;
pub mod tls;
pub mod util;
pub mod version;
pub mod view;
pub mod zfx_id;

use protocol::{Request, Response};

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Dalek(ed25519_dalek::ed25519::Error),
    Sled(sled::Error),
    Actix(actix::MailboxError),

    // client errors
    InvalidResponse,

    // channel errors
    ChannelError(String),
    JoinError,

    // ice errors
    Byzantine,
    Crash,

    // chain errors
    GenesisUndefined,
    InvalidHeight,
    InvalidPredecessor,
    InvalidGenesis,
    InvalidLast,

    /// Error caused by converting from a `String` to an `Id`
    TryFromStringError,
    /// Error when parsing a peer description `ID@IP`
    PeerParseError,

    /// Peer IP and ID don't match or wrong certificate was presented
    UnexpectedPeerConnected,
}

impl std::error::Error for Error {}

impl std::convert::From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::IO(error)
    }
}

impl std::convert::From<ed25519_dalek::ed25519::Error> for Error {
    fn from(error: ed25519_dalek::ed25519::Error) -> Self {
        Error::Dalek(error)
    }
}

impl std::convert::From<sled::Error> for Error {
    fn from(error: sled::Error) -> Self {
        Error::Sled(error)
    }
}

impl std::convert::From<channel::Error<Request, Response>> for Error {
    fn from(error: channel::Error<Request, Response>) -> Self {
        match error {
            channel::Error::IO(io_err) => Error::IO(io_err),
            channel::Error::ReadError(err) => {
                let s = format!("{:?}", err);
                Error::ChannelError(s)
            }
            channel::Error::WriteError(err) => {
                let s = format!("{:?}", err);
                Error::ChannelError(s)
            }
        }
    }
}

impl std::convert::From<channel::Error<Response, Request>> for Error {
    fn from(error: channel::Error<Response, Request>) -> Self {
        match error {
            channel::Error::IO(io_err) => Error::IO(io_err),
            channel::Error::ReadError(err) => {
                let s = format!("{:?}", err);
                Error::ChannelError(s)
            }
            channel::Error::WriteError(err) => {
                let s = format!("{:?}", err);
                Error::ChannelError(s)
            }
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate actix_derive;
extern crate colored;

pub mod chain;
pub mod channel;
pub mod client;
pub mod graph;
pub mod hail;
pub mod ice;
pub mod protocol;
pub mod server;
pub mod sleet;
pub mod util;
pub mod version;
pub mod view;
pub mod zfx_id;
pub mod integration_test;

use protocol::{Request, Response};

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Dalek(ed25519_dalek::ed25519::Error),
    Sled(sled::Error),

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

impl<'a> std::convert::From<channel::Error<'a, Request, Response>> for Error {
    fn from(error: channel::Error<'a, Request, Response>) -> Self {
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

impl<'a> std::convert::From<channel::Error<'a, Response, Request>> for Error {
    fn from(error: channel::Error<'a, Response, Request>) -> Self {
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

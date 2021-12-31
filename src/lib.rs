#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate actix_derive;
extern crate colored;

pub mod util;
pub mod channel;
pub mod version;
pub mod protocol;
pub mod view;
pub mod client;
pub mod server;
pub mod ice;
pub mod chain;

use protocol::{Request, Response};

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
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
    InvalidLast,
}

impl std::error::Error for Error {}

impl std::convert::From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::IO(error)
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
	    channel::Error::IO(io_err) =>
		Error::IO(io_err),
	    channel::Error::ReadError(err) => {
		let s = format!("{:?}", err);
		Error::ChannelError(s)
	    },
	    channel::Error::WriteError(err) => {
		let s = format!("{:?}", err);
		Error::ChannelError(s)
	    },
	}
    }
}

impl<'a> std::convert::From<channel::Error<'a, Response, Request>> for Error {
    fn from(error: channel::Error<'a, Response, Request>) -> Self {
	match error {
	    channel::Error::IO(io_err) =>
		Error::IO(io_err),
	    channel::Error::ReadError(err) => {
		let s = format!("{:?}", err);
		Error::ChannelError(s)
	    },
	    channel::Error::WriteError(err) => {
		let s = format!("{:?}", err);
		Error::ChannelError(s)
	    },
	}
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

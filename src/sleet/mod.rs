mod sleet;
mod sleet_tx;

pub mod conflict_set;

pub use sleet::*;

use crate::alpha::TxHash;
use crate::cell;
use crate::graph;

#[derive(Debug)]
pub enum Error {
    Actix(actix::MailboxError),
    Sled(sled::Error),
    Cell(cell::Error),
    InvalidTxHash(TxHash),
    InvalidConflictSet,
    Graph(graph::Error),
    InsufficientWeight,
}

impl std::error::Error for Error {}

impl std::convert::From<sled::Error> for Error {
    fn from(error: sled::Error) -> Self {
        Error::Sled(error)
    }
}

impl std::convert::From<cell::Error> for Error {
    fn from(error: cell::Error) -> Self {
        Error::Cell(error)
    }
}

impl std::convert::From<graph::Error> for Error {
    fn from(error: graph::Error) -> Self {
        Error::Graph(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

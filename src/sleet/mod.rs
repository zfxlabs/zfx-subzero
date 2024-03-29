//! Sleet is a consensus algorithm based on Avalanche and the closest one to the original papers.
//!
//! The purpose of sleet is to resolve conflicts between cell-based transactions and ensure
//! that a double spending transaction never becomes live, nor adopted in a subsequent block.

mod sleet;
pub mod tx;

pub mod conflict_set;

pub use sleet::*;

use crate::alpha::types::TxHash;
use crate::cell;
use crate::graph;
use crate::storage;

#[derive(Debug)]
pub enum Error {
    Actix(actix::MailboxError),
    Sled(sled::Error),
    Cell(cell::Error),
    Storage(storage::Error),
    /// Coinbase transactions cannot be sent to the mempool
    InvalidCoinbaseTransaction(cell::Cell),
    InvalidTxHash(TxHash),
    InvalidConflictSet,
    Graph(graph::Error),
    InsufficientWeight,
    MissingAncestry,
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

impl std::convert::From<storage::Error> for Error {
    fn from(error: storage::Error) -> Self {
        Error::Storage(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

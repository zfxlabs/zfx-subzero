mod conflict_map;
mod conflict_set;
mod sleet;
mod sleet_tx;
mod spend_map;

pub use sleet::*;

use crate::chain::alpha::tx::{Transaction, TxHash};
use crate::graph;

#[derive(Debug)]
pub enum Error {
    Actix(actix::MailboxError),
    Sled(sled::Error),
    /// Coinbase transactions cannot be sent to the mempool
    InvalidCoinBaseTransaction(Transaction),
    /// Tx is trying to spend invalid UTXOs
    SpendsInvalidUTXOs(Transaction),
    InvalidTransactionHash(TxHash),
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

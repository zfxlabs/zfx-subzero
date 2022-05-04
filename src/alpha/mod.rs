mod alpha;
pub mod types;

pub mod coinbase;
pub mod stake;
pub mod transfer;

pub mod block;

pub mod state;

pub mod initial_staker;

pub use alpha::*;

use crate::cell;
use crate::graph;

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    ActixMailbox,
    Sled(sled::Error),
    Hex(String),
    Bincode(String),
    Dalek(String),
    Cell(cell::Error),
    Graph(graph::Error),
    // Alpha
    BootstrapConsensus,
    // Operations
    UnspendableCell,
    ExceedsAvailableFunds,
    ZeroTransfer,
    ZeroStake,
    InvalidCoinbase,
    InvalidStake,
    // State
    UndefinedCellIds,
    ExistingCellIds,
    ExceedsCapacity,
}

impl std::error::Error for Error {}

impl std::convert::From<sled::Error> for Error {
    fn from(error: sled::Error) -> Self {
        Error::Sled(error)
    }
}

impl std::convert::From<hex::FromHexError> for Error {
    fn from(error: hex::FromHexError) -> Self {
        Error::Hex(format!("{:?}", error))
    }
}

impl std::convert::From<Box<bincode::ErrorKind>> for Error {
    fn from(error: Box<bincode::ErrorKind>) -> Self {
        Error::Bincode(format!("{:?}", error))
    }
}

impl std::convert::From<ed25519_dalek::ed25519::Error> for Error {
    fn from(error: ed25519_dalek::ed25519::Error) -> Self {
        Error::Dalek(format!("{:?}", error))
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

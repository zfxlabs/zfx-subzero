mod tx;
mod spend_map;
mod conflict_set;
mod conflict_map;
mod sleet;

pub use sleet::*;

#[derive(Debug)]
pub enum Error {
    // UndefinedNode(tx::TxHash),
    InvalidTransaction(crate::chain::alpha::tx::Tx),
    InvalidTransactionHash(crate::chain::alpha::tx::TxHash),
    InvalidConflictSet,
    Sled(sled::Error),
}

impl std::error::Error for Error {}

impl std::convert::From<sled::Error> for Error {
    fn from(error: sled::Error) -> Self {
	Error::Sled(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;


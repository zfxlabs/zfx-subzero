pub mod tx;
pub mod block;
pub mod state;

mod initial_staker;
mod alpha;

pub use tx::*;
pub use initial_staker::*;
pub use alpha::*;

use tx::{UTXOId, CoinbaseTx};

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    InvalidCoinbaseInputs(CoinbaseTx),
    InvalidCoinbaseOutputs,
    InvalidUTXO(UTXOId),
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;


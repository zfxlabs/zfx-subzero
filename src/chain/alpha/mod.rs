pub mod block;
pub mod state;
pub mod tx;

mod alpha;
mod initial_staker;

pub use alpha::*;
pub use initial_staker::*;
pub use tx::*;

use tx::{CoinbaseTx, UTXOId};

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

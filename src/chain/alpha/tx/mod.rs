mod coinbase_tx;
mod functions;
mod input;
mod inputs;
mod output;
mod outputs;
mod stake_tx;
mod transaction;
mod transfer_tx;
mod tx;
mod types;
mod utxo_ids;

pub use coinbase_tx::*;
pub use functions::*;
pub use input::*;
pub use inputs::*;
pub use output::*;
pub use outputs::*;
pub use stake_tx::*;
pub use transaction::*;
pub use transfer_tx::*;
pub use tx::*;
pub use utxo_ids::*;

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    ExceedsAvailableFunds,
    ZeroSpend,
    ZeroStake,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

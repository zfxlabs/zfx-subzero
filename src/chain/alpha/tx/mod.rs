mod coinbase_tx;
mod functions;
mod input;
mod output;
mod stake_tx;
mod transaction;
mod transfer_tx;
mod tx;
mod types;

pub use coinbase_tx::*;
pub use functions::*;
pub use input::*;
pub use output::*;
pub use stake_tx::*;
pub use transaction::*;
pub use transfer_tx::*;
pub use tx::*;

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

mod types;
mod functions;
mod input;
mod output;
mod tx;
mod coinbase_tx;
mod stake_tx;
mod transfer_tx;
mod transaction;

pub use functions::*;
pub use input::*;
pub use output::*;
pub use tx::*;
pub use coinbase_tx::*;
pub use stake_tx::*;
pub use transfer_tx::*;
pub use transaction::*;

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    ExceedsAvailableFunds,
    ZeroSpend,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

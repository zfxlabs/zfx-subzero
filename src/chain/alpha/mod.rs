pub mod tx;
pub mod block;
pub mod state;

mod transaction;
mod alpha;

pub use transaction::*;
pub use alpha::*;

#[derive(Debug)]
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


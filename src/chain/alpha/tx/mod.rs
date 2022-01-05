mod stake_tx;
mod tx;

pub use tx::*;
pub use stake_tx::*;

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

//! Cell handling
//!
//! The network is comprised of cell based transactions, which is an extension to the UTXO model
//! to include a `data` field and a cell type. This enables transactions to contain arbitrary data
//! where the cell type defines the interpretation of the cell and is only relevant to clients which support that specific cell type.

mod cell;
mod cell_id;
mod cell_ids;
pub mod cell_operation;
mod cell_type;
mod cell_unlock_script;
pub mod input;
pub mod inputs;
pub mod output;
mod output_index;
pub mod outputs;
pub mod types;

pub use cell::*;
pub use cell_id::*;
pub use cell_ids::*;
pub use cell_type::*;
pub use cell_unlock_script::*;

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Hex(String),
    Bincode(String),
    Dalek(String),
    InvalidCoinbase,
    InvalidStake,
}

impl std::error::Error for Error {}

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

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

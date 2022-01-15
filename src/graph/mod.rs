mod dag;
mod hypergraph;

pub use dag::*;

use crate::chain::alpha::tx::TxHash;

#[derive(Debug)]
pub enum Error {
    VertexExists,
    VacantEntry,
    UndefinedChit,
    UndefinedUTXO,
    ChitReplace,
    DuplicateUTXO,
    DuplicateInputs,
    InvalidTxHash(TxHash),
    EmptyHypergraph,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

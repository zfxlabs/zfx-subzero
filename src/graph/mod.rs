mod dag;

pub mod conflict_graph;
pub mod dependency_graph;

pub use dag::*;

use crate::alpha::types::TxHash;
use crate::cell;
use crate::cell::types::CellHash;

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Cell(cell::Error),
    VertexExists,
    VacantEntry,
    UndefinedChit,
    UndefinedVertex,
    ChitReplace,
    // Dependency graph
    EmptyConflictGraph,
    DuplicateCell,
    UndefinedCell,
    UndefinedCellHash(CellHash),
}

impl std::error::Error for Error {}

impl std::convert::From<cell::Error> for Error {
    fn from(error: cell::Error) -> Self {
        Error::Cell(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

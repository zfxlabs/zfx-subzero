mod conflict_set;
mod hail;
mod height_map;

pub use hail::*;

use crate::chain::alpha::block::{Block, BlockHash};
use crate::graph;

#[derive(Debug)]
pub enum Error {
    Actix(actix::MailboxError),
    Sled(sled::Error),
    InvalidBlock(Block),
    InvalidBlockHash(BlockHash),
    InvalidConflictSet,
    Graph(graph::Error),
    InsufficientWeight,
}

impl std::error::Error for Error {}

impl std::convert::From<sled::Error> for Error {
    fn from(error: sled::Error) -> Self {
        Error::Sled(error)
    }
}

impl std::convert::From<graph::Error> for Error {
    fn from(error: graph::Error) -> Self {
        Error::Graph(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

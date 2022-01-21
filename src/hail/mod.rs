mod block;
mod conflict_map;
mod conflict_set;
mod hail;

pub use hail::*;

use crate::alpha;
use crate::alpha::block::Block;
use crate::alpha::types::{BlockHash, BlockHeight};
use crate::graph;

#[derive(Debug)]
pub enum Error {
    Actix(actix::MailboxError),
    Alpha(alpha::Error),
    Sled(sled::Error),
    Graph(graph::Error),
    InvalidBlock(Block),
    InvalidBlockHash(BlockHash),
    InvalidBlockHeight(BlockHeight),
    InvalidConflictSet,
    InsufficientWeight,
}

impl std::error::Error for Error {}

impl std::convert::From<sled::Error> for Error {
    fn from(error: sled::Error) -> Self {
        Error::Sled(error)
    }
}

impl std::convert::From<alpha::Error> for Error {
    fn from(error: alpha::Error) -> Self {
        Error::Alpha(error)
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

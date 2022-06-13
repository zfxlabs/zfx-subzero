//! Hail is a consensus algorithm based on Snowman but augmented with cryptographic sortition.
//!
//! It is specialised to blocks and ensures that no two conflicting blocks can be accepted at the same height.
//! Similar to [`sleet`][crate::sleet], no inner verification of the block contents nor execution
//! of state transitions is done besides on alpha primitive cells (such as staking cells).


pub mod block;
mod committee;
mod conflict_map;
mod conflict_set;
mod hail;
mod vertex;

pub use hail::*;

use crate::alpha;
use crate::alpha::block::Block;
use crate::alpha::types::{BlockHash, BlockHeight};
use crate::graph;

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    ActixMailboxError,
    Alpha(alpha::Error),
    Sled(sled::Error),
    Graph(graph::Error),
    InvalidBlock(Block),
    InvalidBlockHash(BlockHash),
    InvalidBlockHeight(BlockHeight),
    InvalidParent,
    InvalidConflictSet,
    InsufficientWeight,
    EmptyDAG,
}

impl std::error::Error for Error {}

impl std::convert::From<actix::MailboxError> for Error {
    fn from(_error: actix::MailboxError) -> Self {
        Error::ActixMailboxError
    }
}

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

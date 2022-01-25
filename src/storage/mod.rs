use crate::alpha;
use crate::cell as inner_cell;
use crate::hail;

pub mod block;
pub mod cell;
pub mod hail_block;
pub mod tx;

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Bincode(String),
    Sled(sled::Error),
    Cell(inner_cell::Error),
    Alpha(alpha::Error),
    Hail(hail::Error),
    InvalidGenesis,
    UndefinedGenesis,
    InvalidHeight,
    InvalidPredecessor,
    InvalidLast,
    InvalidCell,
    InvalidTx,
    InvalidHailBlock,
}

impl std::convert::From<Box<bincode::ErrorKind>> for Error {
    fn from(error: Box<bincode::ErrorKind>) -> Self {
        Error::Bincode(format!("{:?}", error))
    }
}

impl std::convert::From<sled::Error> for Error {
    fn from(error: sled::Error) -> Self {
        Error::Sled(error)
    }
}

impl std::convert::From<hail::Error> for Error {
    fn from(error: hail::Error) -> Self {
        Error::Hail(error)
    }
}

impl std::convert::From<inner_cell::Error> for Error {
    fn from(error: inner_cell::Error) -> Self {
        Error::Cell(error)
    }
}

impl std::convert::From<alpha::Error> for Error {
    fn from(error: alpha::Error) -> Self {
        Error::Alpha(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

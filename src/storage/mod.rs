use crate::cell as inner_cell;

pub mod block;
pub mod cell;

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Bincode(String),
    Sled(sled::Error),
    Cell(inner_cell::Error),
    InvalidGenesis,
    UndefinedGenesis,
    InvalidHeight,
    InvalidPredecessor,
    InvalidLast,
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

impl std::convert::From<inner_cell::Error> for Error {
    fn from(error: inner_cell::Error) -> Self {
        Error::Cell(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

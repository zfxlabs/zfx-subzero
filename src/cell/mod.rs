
mod cell;
mod cell_id;
mod cell_type;
mod cell_unlock_script;
mod input;
mod inputs;
mod output;
mod output_index;
mod outputs;
mod types;

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Hex(String),
    Bincode(String),
    Dalek(String),
}

impl std::error::Error for Error {}

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

impl std::convert::From<hex::FromHexError> for Error {
    fn from(error: hex::FromHexError) -> Self {
        Error::Hex(format!("{:?}", error))
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

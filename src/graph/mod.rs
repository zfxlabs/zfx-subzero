mod dag;

pub use dag::*;

#[derive(Debug)]
pub enum Error {
    VertexExists,
    VacantEntry,
    UndefinedChit,
    ChitReplace,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

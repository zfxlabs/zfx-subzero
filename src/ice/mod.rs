//! A `O(1`) reservoir sampling based consensus algorithm for transiently establishing the liveness
//! of peers and performing a safe network bootstrap.
mod choice;
mod constants;
mod ice;
mod query;
mod quorum;
mod reservoir;

pub mod dissemination;

pub use choice::Choice;
pub use constants::*;
pub use ice::*;
pub use reservoir::Reservoir;

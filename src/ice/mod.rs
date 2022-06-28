//! A `O(1`) reservoir sampling based consensus algorithm for transiently establishing the liveness
//! of peers and performing a safe network bootstrap.
//!
//! `ice` defines a reservoir sampling based consensus mechanism and a gossip protocol
//! for performing a safe (trusted / weightless) bootstrap of the [`alpha`][crate::alpha] chain.
//! The weightless aspect of the algorithm means that it is not resistant to sybil attacks
//! and therefore requires a trusted whitelist initially.
//!
//! `ice` is augmented with sybil resistance once the alpha chain is bootstrapped, which defines the validator set.
//!
//! The code is organised around the [`Ice`] actor and its messages. The [`run`] function drives the protocol rounds in a loop.

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
pub use query::Query;
pub use reservoir::Reservoir;

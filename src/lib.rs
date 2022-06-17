#![doc(html_logo_url = "/logo.png")]

//! # Subzero
//!
//! Subzero is an ensemble of components for the creation of a network inspired by the `Snow*`
//! family of consensus algorithms.
//!
//! Although there is a lot of surrounding code for setting up secure channels, protocol messages,
//! bootstrapping etc, `subzero` is the synthesis of four main actors - `alpha`, `ice`, `sleet` and
//! `hail`.
//!
//! ## Alpha
//!
//! `alpha` is the primary chain protocol which defines staking operations and basic transfers. It is
//! defined as an actor which can be used to initialise and retrieve chain state.
//!
//! `alpha` is responsible for providing the latest known set of cells, their ancestry and the state
//! which relates to the latest application of said cells.
//!
//! ## Ice
//!
//! `ice` is a reservoir-sampling based consensus algorithm for approximating the liveness of the
//! validator set dynamically. `ice` is required in order to reward validators based on their uptime.
//!
//! ## Sleet
//!
//! `sleet` is a cell-based (equivalent to a utxo transaction with added data / type field) consensus
//! algorithm which uses a directed acyclic graph and random sampling to reach consensus.
//!
//! `sleet` is based on `Avalanche` with improvements against safety faults[1].
//!
//! Although some of the algorithms have been optimised in `sleet` to be constant space, the
//! underlying graph (data storage) still needs to be optimised such that they use a constant amount
//! of memory.
//!
//! ## Hail
//!
//! `hail` is a block-based consensus algorithm which uses a directed acyclic graph with random
//! sampling to reach consensus, as well as VRF based sortition to select block producers, where the
//! lowest VRF hash is used to resolve conflicts.
//!
//! Although `hail` tries to successively collect enough samples to advance, it is currently lacking
//! an extra instance of consensus in its decision making process. This is because initially we
//! assumed the `sleet` model would work almost unchanged with blocks.

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate actix_derive;
extern crate colored;

pub mod message;
pub mod p2p;
pub mod protocol;
pub mod server;
pub mod util;

mod integration_test;

pub mod cell;
pub mod channel;
pub mod client;
pub mod graph;
pub mod hail;

pub mod ice;
pub mod porter;
pub mod sleet;
pub mod storage;
pub mod tls;
pub mod view;

pub mod alpha;

use serde::{de::DeserializeOwned, Serialize};

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Dalek(ed25519_dalek::ed25519::Error),
    Sled(sled::Error),
    Actix(actix::MailboxError),

    // client errors
    InvalidResponse,

    // router errors
    Bootstrapping,
    IceUninitialised,
    UnknownRequest,

    // channel errors
    ChannelError(String),
    JoinError,

    // ice errors
    Byzantine,
    Crash,

    // chain errors
    GenesisUndefined,
    InvalidHeight,
    InvalidPredecessor,
    InvalidGenesis,
    InvalidLast,

    /// Error caused by converting from a `String` to an `Id`
    TryFromStringError,
    /// Error when parsing a peer description `ID@IP`
    PeerParseError,

    /// Peer IP and ID don't match or wrong certificate was presented
    UnexpectedPeerConnected,

    // p2p errors
    ActixMailboxError,
    UnexpectedState,
    UnexpectedPeer,
    PeerListOverflow,
    EmptyResponse,
    EmptyConnection,
    Timeout,
    IncompatibleVersion,
    MulticastOverflow,
}

impl std::error::Error for Error {}

impl std::convert::From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::IO(error)
    }
}

impl std::convert::From<actix::MailboxError> for Error {
    fn from(error: actix::MailboxError) -> Self {
        Error::ActixMailboxError
    }
}

impl std::convert::From<ed25519_dalek::ed25519::Error> for Error {
    fn from(error: ed25519_dalek::ed25519::Error) -> Self {
        Error::Dalek(error)
    }
}

impl std::convert::From<sled::Error> for Error {
    fn from(error: sled::Error) -> Self {
        Error::Sled(error)
    }
}

impl<Req, Rsp> std::convert::From<channel::Error<Req, Rsp>> for Error
where
    Req: Serialize + DeserializeOwned,
    Rsp: Serialize + DeserializeOwned,
{
    fn from(error: channel::Error<Req, Rsp>) -> Self {
        match error {
            channel::Error::IO(io_err) => Error::IO(io_err),
            channel::Error::ReadError(err) => {
                let s = format!("{:?}", err);
                Error::ChannelError(s)
            }
            channel::Error::WriteError(err) => {
                let s = format!("{:?}", err);
                Error::ChannelError(s)
            }
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

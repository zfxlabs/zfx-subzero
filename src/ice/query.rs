use crate::p2p::peer_meta::PeerMetadata;

use crate::colored::Colorize;
use crate::ice::Choice;

use std::net::SocketAddr;

/// A `Query` is a question to another peer carrying our current consensus choice about
/// that peer.
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct Query {
    pub peer_meta: PeerMetadata,
    pub choice: Choice,
}

impl std::fmt::Debug for Query {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "QUERY({}, {:?})", format!("{:?}", self.peer_meta).yellow(), self.choice,)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct Outcome {
    pub peer_meta: PeerMetadata,
    pub choice: Choice,
}

impl Outcome {
    pub fn new(peer_meta: PeerMetadata, choice: Choice) -> Outcome {
        Outcome { peer_meta, choice }
    }
}

impl std::fmt::Debug for Outcome {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            fmt,
            "OUTCOME({}, {:?})",
            format!("{:?}", self.peer_meta).yellow(),
            self.choice.clone(),
        )
    }
}

//! The network version message definition.
use crate::p2p::id::Id;
use crate::p2p::peer_meta::PeerMetadata;

use std::collections::HashSet;
use std::net::SocketAddr;

pub const CURRENT_VERSION: Id = Id([0u8; 32]);

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "VersionAck")]
pub struct Version {
    pub peer: PeerMetadata,
    pub peer_set: HashSet<PeerMetadata>,
    pub version: Id,
}

impl Version {
    pub fn new(peer: PeerMetadata, peer_set: HashSet<PeerMetadata>) -> Self {
        Version { peer, peer_set, version: CURRENT_VERSION }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct VersionAck {
    pub peer: PeerMetadata,
    pub peer_set: HashSet<PeerMetadata>,
    pub version: Id,
}

impl VersionAck {
    pub fn new(peer: PeerMetadata, peer_set: HashSet<PeerMetadata>) -> Self {
        VersionAck { peer, peer_set, version: CURRENT_VERSION }
    }
}

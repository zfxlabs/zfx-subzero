use crate::ice::query::{Outcome, Query};
use crate::p2p::peer_meta::PeerMetadata;

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "PingAck")]
pub struct Ping {
    pub peer_meta: PeerMetadata,
    pub queries: Vec<Query>,
}

impl Ping {
    pub fn new(peer_meta: PeerMetadata, queries: Vec<Query>) -> Self {
        Ping { peer_meta, queries }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct PingAck {
    pub peer_meta: PeerMetadata,
    pub outcomes: Vec<Outcome>,
}

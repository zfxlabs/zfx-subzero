use crate::zfx_id::Id;

use std::net::SocketAddr;

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct PeerMetadata {
    pub id: Id,
    pub ip: SocketAddr,
    pub chains: Vec<Id>,
}

impl PeerMetadata {
    pub fn new(id: Id, ip: SocketAddr, chains: Vec<Id>) -> Self {
        PeerMetadata { id, ip, chains }
    }
}

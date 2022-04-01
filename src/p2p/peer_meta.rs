use crate::zfx_id::Id;

use std::net::SocketAddr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerMetadata {
    pub id: Id,
    pub ip: SocketAddr,
}

impl PeerMetadata {
    pub fn new(id: Id, ip: SocketAddr) -> Self {
        PeerMetadata { id, ip }
    }
}

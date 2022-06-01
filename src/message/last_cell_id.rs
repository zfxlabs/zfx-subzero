//! The last cell id message definition.
use crate::p2p::id::Id;
use crate::p2p::peer_meta::PeerMetadata;
use crate::cell::CellId;

use std::collections::HashSet;
use std::net::SocketAddr;

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "LastCellIdAck")]
pub struct LastCellId {
    pub peer: PeerMetadata,
}

impl LastCellId {
    pub fn new(peer: PeerMetadata) -> Self {
        LastCellId { peer }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct LastCellIdAck {
    pub peer: PeerMetadata,
    pub last_cell_id: CellId,
}

impl LastCellIdAck {
    pub fn new(peer: PeerMetadata, last_cell_id: CellId) -> Self {
        LastCellIdAck { peer, last_cell_id }
    }
}

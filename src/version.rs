//! Messages for querying and replying with the node version

use crate::zfx_id::Id;
use std::net::SocketAddr;

/// Query the version of the other node.
///
/// See [Request][crate::protocol::Request]
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "VersionAck")]
pub struct Version {
    pub id: Id,
    pub ip: SocketAddr,
}

/// Reply to  a [Version] query
///
/// See [Response][crate::protocol::Response]
#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct VersionAck {
    pub id: Id,
    pub ip: SocketAddr,
    pub peer_list: Vec<(Id, SocketAddr)>,
}

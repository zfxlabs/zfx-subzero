use zfx_id::Id;
use std::net::SocketAddr;

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "VersionAck")]
pub struct Version {
    pub id: Id,
    pub ip: SocketAddr,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct VersionAck {
    pub ip: SocketAddr,
    pub peer_list: Vec<(Id, SocketAddr)>,
}

use crate::version;
use crate::ice;
use crate::chain::alpha;

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Response")]
pub enum Request {
    // Handshake
    Version(version::Version),
    // Ice
    Ping(ice::Ping),
    // Chain Bootstrapping
    GetLastAccepted,
    GetAncestors,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub enum Response {
    // Handshake
    VersionAck(version::VersionAck),
    // Ice
    Ack(ice::Ack),
    // Chain Bootstrapping
    LastAccepted(alpha::LastAccepted),
    Ancestors,
    // Error
    Unknown,
}


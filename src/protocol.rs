use crate::version;
use crate::ice;
use crate::chain::alpha;
use crate::sleet;
use crate::hail;

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
    // Sleet
    ReceiveTx(sleet::ReceiveTx),
    QueryTx(sleet::QueryTx),
    // Hail
    QueryBlock(hail::QueryBlock),
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
    // Sleet
    ReceiveTxAck(sleet::ReceiveTxAck),
    QueryTxAck(sleet::QueryTxAck),
    // Hail
    QueryBlockAck(hail::QueryBlockAck),
    // Error
    Unknown,
}


use crate::chain::alpha;
use crate::chain::alpha::TxHash;
use crate::hail;
use crate::ice;
use crate::sleet;
use crate::version;

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
    // State
    GetTransactions,
    // Sleet
    GetTx(sleet::GetTx),
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
    Transactions(sleet::Transactions),
    // Sleet
    TxAck(sleet::TxAck),
    ReceiveTxAck(sleet::ReceiveTxAck),
    QueryTxAck(sleet::QueryTxAck),
    // Hail
    QueryBlockAck(hail::QueryBlockAck),
    // Error
    Unknown,
}

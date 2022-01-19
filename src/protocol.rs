use crate::alpha;
use crate::alpha::TxHash;
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
    GetCellHashes,
    // Sleet
    GetCell(sleet::GetCell),
    GenerateTx(sleet::GenerateTx),
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
    CellHashes(sleet::CellHashes),
    // Sleet
    CellAck(sleet::CellAck),
    GenerateTxAck(sleet::GenerateTxAck),
    QueryTxAck(sleet::QueryTxAck),
    // Hail
    QueryBlockAck(hail::QueryBlockAck),
    // Error
    Unknown,
}

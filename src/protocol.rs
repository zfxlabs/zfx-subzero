use crate::alpha;
use crate::alpha::types::TxHash;
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
    CheckStatus,
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
    GetTxAncestors(sleet::GetTxAncestors),
    // Hail
    GetBlock(hail::GetBlock),
    GetBlockByHeight(hail::GetBlockByHeight),
    QueryBlock(hail::QueryBlock),
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub enum Response {
    // Handshake
    VersionAck(version::VersionAck),
    // Ice
    Ack(ice::Ack),
    Status(ice::Status),
    // Chain Bootstrapping
    LastAccepted(alpha::LastAccepted),
    Ancestors,
    CellHashes(sleet::CellHashes),
    // Sleet
    CellAck(sleet::CellAck),
    GenerateTxAck(sleet::GenerateTxAck),
    QueryTxAck(sleet::QueryTxAck),
    TxAncestors(sleet::TxAncestors),
    // Hail
    BlockAck(hail::BlockAck),
    QueryBlockAck(hail::QueryBlockAck),
    // Error
    Unknown,
}

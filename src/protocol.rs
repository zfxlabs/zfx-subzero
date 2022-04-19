use crate::alpha;
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
    GetNodeStatus,
    // State
    GetCellHashes,
    GetAcceptedCellHashes,
    // Sleet
    GetCell(sleet::GetCell),
    GetAcceptedCell(sleet::sleet_cell_handlers::GetAcceptedCell),
    GenerateTx(sleet::GenerateTx),
    QueryTx(sleet::QueryTx),
    GetTxAncestors(sleet::GetTxAncestors),
    GetAcceptedFrontier,
    FetchTx(sleet::FetchTx),
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
    // Chain Bootstrapping
    LastAccepted(alpha::LastAccepted),
    Ancestors,
    CellHashes(sleet::CellHashes),
    AcceptedCellHashes(sleet::sleet_cell_handlers::AcceptedCellHashes),
    NodeStatus(alpha::status_handler::NodeStatus),
    // Sleet
    CellAck(sleet::CellAck),
    AcceptedCellAck(sleet::sleet_cell_handlers::AcceptedCellAck),
    GenerateTxAck(sleet::GenerateTxAck),
    QueryTxAck(sleet::QueryTxAck),
    TxAncestors(sleet::TxAncestors),
    AcceptedFrontier(sleet::AcceptedFrontier),
    FetchedTx(sleet::FetchedTx),
    // Hail
    BlockAck(hail::BlockAck),
    QueryBlockAck(hail::QueryBlockAck),
    // Error
    Unknown,
    /// Refuse a validator-only request from a non-validator
    RequestRefused,
}

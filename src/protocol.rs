//! The node network message protocol.

use crate::alpha;
use crate::hail;
use crate::ice;
use crate::message;
use crate::sleet;

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Response")]
pub enum Request {
    // Handshake
    Version(message::Version),
    // Primary Network
    LastCellId(message::LastCellId),
    // Ice
    Ping(message::Ping),
    CheckStatus,
    // Chain Bootstrapping
    GetLastAccepted,
    GetAncestors,
    // State
    GetCellHashes,
    GetAcceptedCellHashes,
    // Sleet
    GetCell(sleet::GetCell),
    GetAcceptedCell(sleet::sleet_cell_handlers::GetAcceptedCell),
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
    VersionAck(message::VersionAck),
    // Primary Network
    LastCellIdAck(message::LastCellIdAck),
    // Ice
    PingAck(message::PingAck),
    Status(ice::Status),
    // Alpha
    AncestorsAck,
    // LastAccepted(alpha::LastAccepted),
    // Ancestors,
    CellHashes(sleet::CellHashes),
    AcceptedCellHashes(sleet::sleet_cell_handlers::AcceptedCellHashes),
    // Sleet
    CellAck(sleet::CellAck),
    AcceptedCellAck(sleet::sleet_cell_handlers::AcceptedCellAck),
    GenerateTxAck(sleet::GenerateTxAck),
    QueryTxAck(sleet::QueryTxAck),
    TxAncestors(sleet::TxAncestors),
    // Hail
    BlockAck(hail::BlockAck),
    QueryBlockAck(hail::QueryBlockAck),
    // Error
    Unknown,
    /// Refuse a validator-only request from a non-validator
    RequestRefused,
}

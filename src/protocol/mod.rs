//! The node network message protocol.

pub mod chain;
pub mod graph;
pub mod network;

use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

pub trait Request: 'static + Unpin + Clone + Serialize + DeserializeOwned + Send + Debug {}
pub trait Response: 'static + Unpin + Clone + Serialize + DeserializeOwned + Send + Debug {}

//use crate::p2p::types;

//impl types::Request for Request {}
//impl types::Response for Request {}
//impl types::Request for Response {}
//impl types::Response for Response {}

// #[derive(Debug, Clone, Serialize, Deserialize, Message)]
// #[rtype(result = "Response")]
// pub enum Request {
//     // Handshake
//     Version(message::Version),
//     // Primary Network
//     LastCellId(message::LastCellId),
//     // Ice
//     Ping(message::Ping),
//     CheckStatus,
//     // Chain Bootstrapping
//     GetLastAccepted,
//     GetAncestors,
//     // State
//     GetCellHashes,
//     GetAcceptedCellHashes,
//     // Sleet
//     // GetCell(sleet::GetCell),
//     // GetAcceptedCell(sleet::sleet_cell_handlers::GetAcceptedCell),
//     // GenerateTx(sleet::GenerateTx),
//     // QueryTx(sleet::QueryTx),
//     // GetTxAncestors(sleet::GetTxAncestors),
//     // Hail
//     GetBlock(hail::GetBlock),
//     GetBlockByHeight(hail::GetBlockByHeight),
//     QueryBlock(hail::QueryBlock),
// }

// #[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
// pub enum Response {
//     // Handshake
//     VersionAck(message::VersionAck),
//     // Primary Network
//     LastCellIdAck(message::LastCellIdAck),
//     // Ice
//     PingAck(message::PingAck),
//     Status(ice::Status),
//     // Alpha
//     AncestorsAck,
//     // LastAccepted(alpha::LastAccepted),
//     // Ancestors,
//     // CellHashes(sleet::CellHashes),
//     // AcceptedCellHashes(sleet::sleet_cell_handlers::AcceptedCellHashes),
//     // Sleet
//     // CellAck(sleet::CellAck),
//     // AcceptedCellAck(sleet::sleet_cell_handlers::AcceptedCellAck),
//     // GenerateTxAck(sleet::GenerateTxAck),
//     // QueryTxAck(sleet::QueryTxAck),
//     // TxAncestors(sleet::TxAncestors),
//     // Hail
//     BlockAck(hail::BlockAck),
//     QueryBlockAck(hail::QueryBlockAck),
//     // Error
//     Bootstrapping,
//     IceUninitialised,
//     Unknown,
//     /// Refuse a validator-only request from a non-validator
//     RequestRefused,
// }

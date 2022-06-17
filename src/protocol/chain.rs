//! Defines graph based protocol messages.
use super::{Request, Response};
use crate::message;

impl Request for ChainRequest {}
impl Response for ChainResponse {}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Result<(), crate::Error>")]
pub enum ChainRequest {
    Query(message::QueryBlock),
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub enum ChainResponse {
    QueryAck(message::QueryBlockAck),
}

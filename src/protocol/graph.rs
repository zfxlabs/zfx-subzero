//! Defines graph based protocol messages.
use super::{Request, Response};
use crate::message;

impl Request for GraphRequest {}
impl Response for GraphResponse {}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Result<(), crate::Error>")]
pub enum GraphRequest {
    LastCellId(message::LastCellId),
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub enum GraphResponse {
    LastCellIdAck(message::LastCellIdAck),
}

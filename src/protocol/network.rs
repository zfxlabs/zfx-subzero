//! Defines network protocol messages.
use super::graph;
use super::{Request, Response};
use crate::message;

impl Request for NetworkRequest {}
impl Response for NetworkResponse {}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Result<(), crate::Error>")]
pub enum NetworkRequest {
    // Handshake
    Version(message::Version),
    // Ice
    Ping(message::Ping),
    // Graph
    GraphRequest(graph::GraphRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub enum NetworkResponse {
    // Handshake
    VersionAck(message::VersionAck),
    // Ice
    PingAck(message::PingAck),
    // Graph
    GraphResponse(graph::GraphResponse),
}

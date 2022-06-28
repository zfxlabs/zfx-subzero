//! TLS and certificate-related code, to be used in a peer-to-peer context
//!
//! Nodes use self-signed certificates to identify themselves on the network---the node ID is derived from
//! the certificate presented by the node. Consequently, both nodes, acting as the client and the server,
//! have to present their certificates, in order to establish a connection.

pub mod certificate;
pub mod connection_stream;
pub mod tls;
pub mod upgrader;

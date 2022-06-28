//! NAT traversal
//!
//! Wraps the `rust-igd` library for basic functions to add, refresh and remove port mappings.
//!
//! ## Use
//!
//! - Construct the [`Mapper`][mapper_handler::Mapper] struct with the local address and the optional `RouterConfig` parameter.
//! - Provide a correct SSDP broadcast address with [`RouterConfig`][params::RouterConfig] if upnp gateway retrieval is unsuccessful.
//! - If mapping is successful, it returns the newly mapped entry
//! - To dinamically refresh port lease, call `refresh_mapping` with the `add_port_mapping`] return value and the mapping refresh interval
mod gateway;
mod mapper_actor;
pub mod mapper_handler;
mod messages;
pub mod params;

pub use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use self::params::{AddMappingEntry, PortMappingEntry};

#[derive(Debug)]
pub enum Error {
    GatewayRetrieve(igd::SearchError),
    PortAdd(igd::AddPortError),
    ExternalIpRetrieve(igd::GetExternalIpError),
    PortRemove(igd::RemovePortError),
    ExternalIpChanged(String),
    MappingRefresh(String),
}

impl std::convert::From<igd::SearchError> for Error {
    fn from(error: igd::SearchError) -> Error {
        Error::GatewayRetrieve(error)
    }
}

impl std::convert::From<igd::AddPortError> for Error {
    fn from(error: igd::AddPortError) -> Error {
        Error::PortAdd(error)
    }
}

impl std::convert::From<igd::GetExternalIpError> for Error {
    fn from(error: igd::GetExternalIpError) -> Error {
        Error::ExternalIpRetrieve(error)
    }
}

impl std::convert::From<igd::RemovePortError> for Error {
    fn from(error: igd::RemovePortError) -> Error {
        Error::PortRemove(error)
    }
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Represents the protocols available for port mapping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Protocol {
    TCP,
    UDP,
}

impl From<PortMappingEntry> for AddMappingEntry {
    fn from(pme: PortMappingEntry) -> AddMappingEntry {
        AddMappingEntry::new(
            pme.local_address,
            pme.external_port,
            pme.protocol,
            pme.lease_duration,
            pme.node_description.to_string(),
        )
    }
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                Protocol::TCP => "TCP",
                Protocol::UDP => "UDP",
            }
        )
    }
}

impl From<Protocol> for igd::PortMappingProtocol {
    fn from(protocol: Protocol) -> igd::PortMappingProtocol {
        match protocol {
            Protocol::TCP => igd::PortMappingProtocol::TCP,
            Protocol::UDP => igd::PortMappingProtocol::UDP,
        }
    }
}

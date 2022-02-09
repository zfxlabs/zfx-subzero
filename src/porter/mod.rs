mod mapper_actor;
pub mod mapper_handler;
mod messages;
mod params;

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
pub enum PortMappingProtocol {
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

impl std::fmt::Display for PortMappingProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                PortMappingProtocol::TCP => "TCP",
                PortMappingProtocol::UDP => "UDP",
            }
        )
    }
}

impl From<PortMappingProtocol> for igd::PortMappingProtocol {
    fn from(protocol: PortMappingProtocol) -> igd::PortMappingProtocol {
        match protocol {
            PortMappingProtocol::TCP => igd::PortMappingProtocol::TCP,
            PortMappingProtocol::UDP => igd::PortMappingProtocol::UDP,
        }
    }
}

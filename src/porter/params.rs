use std::{
    net::{Ipv4Addr, SocketAddrV4},
    time::Duration,
};

use igd::SearchOptions;

use crate::porter::Protocol;

/// Represents a port mapping result
#[derive(Debug, Clone)]
pub struct PortMappingEntry {
    /// This address is where the traffic is sent to
    pub local_address: SocketAddrV4,
    /// The external address traffic will go through
    pub external_port: u16,
    /// Represents the protocols available for port mapping.
    pub protocol: Protocol,
    /// Duration the lease is aquired for
    pub lease_duration: Duration,
    /// description of the mapping entry
    pub node_description: String,
}

/// Represents the parmeters required for port binding
#[derive(Debug, Clone)]
pub struct AddMappingEntry {
    /// This address is where the traffic is sent to
    pub local_address: SocketAddrV4,
    /// The external address traffic will go through
    pub external_port: u16,
    /// Represents the protocols available for port mapping.
    pub protocol: Protocol,
    /// Duration the lease is aquired for
    pub lease_duration: Duration,
    /// description of the mapping entry
    pub node_description: String,
}

impl AddMappingEntry {
    pub fn new(
        local_address: SocketAddrV4,
        external_port: u16,
        protocol: Protocol,
        duration: Duration,
        node_desc: String,
    ) -> Self {
        Self {
            local_address: local_address,
            external_port: external_port,
            protocol: protocol,
            lease_duration: duration,
            node_description: node_desc,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RefreshMappingEntry {
    /// Port mapping is refresh every [mapping_update_interval]
    pub mapping_update_interval: Duration,
    pub external_ip: Ipv4Addr,
    pub add_params: AddMappingEntry,
}

impl RefreshMappingEntry {
    pub fn new(
        update_interval: Duration,
        external_ip: Ipv4Addr,
        add_params: AddMappingEntry,
    ) -> Self {
        Self {
            mapping_update_interval: update_interval,
            external_ip: external_ip,
            add_params: add_params,
        }
    }
}

/// Contains the parameters for the IGD gateway search
#[derive(Debug, Clone, Copy)]
pub struct RouterConfig {
    /// Bind address for UDP socket (defaults to all `0.0.0.0`)
    pub bind_addr: SocketAddrV4,
    /// Broadcast address for network discovery (defaults to '239.255.255.250')
    pub broadcast_addr: SocketAddrV4,
    /// Timeout for gateway search
    pub search_timeout: Option<Duration>,
}

impl RouterConfig {
    pub fn new(
        bind_addr: SocketAddrV4,
        broadcast_addr: SocketAddrV4,
        search_timeout: Option<Duration>,
    ) -> Self {
        Self {
            bind_addr: bind_addr,
            broadcast_addr: broadcast_addr,
            search_timeout: search_timeout,
        }
    }
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0),
            broadcast_addr: "239.255.255.250:1900".parse().unwrap(),
            search_timeout: Some(Duration::from_secs(10)),
        }
    }
}

impl From<RouterConfig> for SearchOptions {
    fn from(options: RouterConfig) -> SearchOptions {
        SearchOptions {
            bind_addr: options.bind_addr.into(),
            broadcast_address: options.broadcast_addr.into(),
            timeout: options.search_timeout,
        }
    }
}

pub struct NetworkConfig {
    pub local_address: SocketAddrV4,
    //pub external_address: SocketAddrV4,
}

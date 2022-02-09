use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use actix::{Actor, Addr};
use tracing::info;

use crate::porter::messages::MappingMessage;
use crate::porter::params::{AddMappingEntry, PortMappingEntry};

use super::mapper_actor::MapperActor;
use super::messages::GetExternalIpMessage;
use super::params::{NetworkConfig, RefreshMappingEntry, RouterConfig};
use super::{Error, PortMappingProtocol};

const MAPPING_LEASE_DURATION: Duration = Duration::from_secs(3600 * 24);

/// Handles mapping functions such as add/refresh/remove/get external ip
pub struct Mapper {
    /// Network config of the node containing local and external addresses
    network_config: NetworkConfig,
    /// Mapper actor mailbox address
    mapper_actor: Addr<MapperActor>,
    /// Refresher actor mailbox address
    refresher_actor: Addr<MapperActor>,
}

impl Mapper {
    pub fn new(local_addr: SocketAddr, router_config: Option<RouterConfig>) -> Self {
        Self {
            network_config: NetworkConfig {
                local_address: match local_addr {
                    SocketAddr::V4(addr) => addr,
                    SocketAddr::V6(_addr) => panic!("Expected SocketAddrV4, got V6!"),
                },
            },
            mapper_actor: MapperActor {
                config: match router_config {
                    Some(conf) => conf,
                    None => RouterConfig::default(),
                },
            }
            .start(),
            refresher_actor: MapperActor {
                config: match router_config {
                    Some(conf) => conf,
                    None => RouterConfig::default(),
                },
            }
            .start(),
        }
    }

    async fn add_port_mapping(
        &mut self,
        external_port: u16,
        protocol: PortMappingProtocol,
        node_desc: &str,
    ) -> Result<PortMappingEntry, Error> {
        let add_params = AddMappingEntry::new(
            self.network_config.local_address,
            external_port,
            protocol,
            MAPPING_LEASE_DURATION,
            node_desc.to_string(),
        );

        info!("Mapping {} to external port {}", self.network_config.local_address, external_port);
        let _ =
            self.mapper_actor.send(MappingMessage::AddMapping(add_params.clone())).await.unwrap();

        Ok(PortMappingEntry {
            local_address: add_params.local_address,
            node_description: add_params.node_description.to_string(),
            external_port: add_params.external_port,
            lease_duration: add_params.lease_duration,
            protocol: add_params.protocol,
        })
    }

    /// Returns the external IP of the attached IGD
    async fn get_external_ip(&self) -> Result<Ipv4Addr, Error> {
        let resp = self.mapper_actor.send(GetExternalIpMessage::GetExternalIp).await.unwrap();

        resp
    }

    /// Refreshes port mapping periodically
    async fn refresh_mapping(
        &self,
        port_mapping: PortMappingEntry,
        external_ip: Ipv4Addr,
        mapping_update_interval: Duration,
    ) -> Result<(), Error> {
        let refresh_params =
            RefreshMappingEntry::new(mapping_update_interval, external_ip, port_mapping.into());

        // TODO: use channels to get information from the actor maybe? Is logging inside actor enough?
        let resp = self
            .refresher_actor
            .send(MappingMessage::RefreshMapping(refresh_params))
            .await
            .unwrap();

        resp
    }

    pub async fn add_and_refresh_mapping(
        &mut self,
        external_port: u16,
        protocol: PortMappingProtocol,
        node_desc: &str,
    ) -> Result<(), Error> {
        let add_res = self.add_port_mapping(external_port, protocol, node_desc).await;

        match add_res.as_ref() {
            Ok(_result) => println!("Port mapping succeeded"),
            Err(err) => println!("Port mapping failed: {}", err),
        }

        let external_ip = self.get_external_ip().await.unwrap();

        let _ = self.refresh_mapping(add_res.unwrap(), external_ip, Duration::from_secs(120)).await;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[actix_rt::test]
    async fn add_mapping() {
        use std::net::IpAddr;

        let mut mapper =
            Mapper::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 109)), 24567), None);

        let res = mapper
            .add_port_mapping(
                24568,
                PortMappingProtocol::TCP,
                "zfx_node_handler_test_add_and_refresh",
            )
            .await;

        assert!(res.is_ok());
    }

    #[actix_rt::test]
    async fn get_external_ip() {
        use std::net::IpAddr;

        let mapper =
            Mapper::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 109)), 24567), None);

        let res = mapper.get_external_ip().await;

        assert!(res.is_ok());
    }

    #[actix_rt::test]
    async fn add_and_refresh() {
        use std::net::IpAddr;

        let mut mapper =
            Mapper::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 50, 1)), 24567), None);

        let res = mapper
            .add_port_mapping(
                25000,
                PortMappingProtocol::TCP,
                "zfx_node_handler_test_add_and_refresh1",
            )
            .await;

        let refresh_res = mapper
            .refresh_mapping(res.unwrap(), Ipv4Addr::new(192, 168, 50, 1), Duration::from_secs(1))
            .await;

        let res2 = mapper
            .add_port_mapping(
                25001,
                PortMappingProtocol::TCP,
                "zfx_node_handler_test_add_and_refresh2",
            )
            .await;

        let refresh_res2 = mapper
            .refresh_mapping(res2.unwrap(), Ipv4Addr::new(192, 168, 50, 1), Duration::from_secs(1))
            .await;

        assert!(refresh_res.is_ok());
        assert!(refresh_res2.is_ok());
    }
}

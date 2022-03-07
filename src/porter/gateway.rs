use igd::SearchOptions;
use std::net::Ipv4Addr;

use super::{
    params::{AddMappingEntry, RouterConfig},
    Error,
};

pub struct Gateway {
    gw: igd::Gateway,
}

impl Gateway {
    pub fn new(config: RouterConfig) -> Gateway {
        Gateway { gw: igd::search_gateway(SearchOptions::from(config)).unwrap() }
    }

    pub fn add_mapping(&self, add_params: AddMappingEntry) -> Result<(), Error> {
        self.gw.add_port(
            add_params.protocol.into(),
            add_params.external_port,
            add_params.local_address,
            add_params.lease_duration.as_secs() as u32,
            &add_params.node_description,
        )?;

        Ok(())
    }

    pub fn get_external_ip(&self) -> Result<Ipv4Addr, Error> {
        let ext_ip = self.gw.get_external_ip()?;

        Ok(ext_ip)
    }
}

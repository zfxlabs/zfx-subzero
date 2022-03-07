use std::net::Ipv4Addr;

use actix::Message;

use super::params::{AddMappingEntry, RefreshMappingEntry};
use super::Error;

#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<(), Error>")]
pub enum MappingMessage {
    AddMapping(AddMappingEntry),
    RefreshMapping(RefreshMappingEntry),
}

#[derive(Debug, Clone, Message)]
#[rtype(result = "Result<Ipv4Addr, Error>")]
pub enum GetExternalIpMessage {
    GetExternalIp,
}

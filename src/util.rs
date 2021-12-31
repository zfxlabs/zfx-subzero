use zfx_id::Id;
use std::net::SocketAddr;

pub fn id_from_ip(ip: &SocketAddr) -> Id {
    Id::new(format!("{:?}", ip.clone()).as_bytes())
}

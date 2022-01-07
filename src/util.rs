use crate::zfx_id::Id;
use std::net::SocketAddr;

/// Converts a `SocketAddr` into an *untrusted* identity.
pub fn id_from_ip(ip: &SocketAddr) -> Id {
    Id::new(format!("{:?}", ip.clone()).as_bytes())
}

/// Compute the `hail` consensus weight based on the number of tokens a validator has.
pub fn percent_of(qty: u64, total: u64) -> f64 {
    qty as f64 / total as f64
}

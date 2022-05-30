use crate::p2p::peer_meta;
use crate::{Error, Result};

use super::id::Id;

use std::net::SocketAddr;
use std::hash::{Hash, Hasher};
use std::collections::HashSet;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PeerMetadata {
    /// The `public_key_hash` of the peer for TLS or the hash of `id:ip` for plain TCP.
    pub id: Id,
    /// The peers ip address.
    pub ip: SocketAddr,
    /// The chains which the peer uses.
    pub chains: HashSet<Id>,
}

impl Hash for PeerMetadata {
    fn hash<H: Hasher>(&self, state: &mut H) {
	self.id.hash(state);
	self.ip.hash(state);
        let mut chains: Vec<Id> = self.chains.iter().cloned().collect();
        chains.sort();
        chains.hash(state);
    }
}

fn collect_chains(chains: Vec<Id>) -> HashSet<Id> {
    chains.iter().cloned().collect::<HashSet<Id>>()
}    

impl PeerMetadata {
    pub fn new(id: Id, ip: SocketAddr, chains: Vec<Id>) -> Self {
        PeerMetadata { id, ip, chains: collect_chains(chains) }
    }

    /// Parse a peer description from the format `IP` or `ID@IP` to its ID and address
    pub fn from_id_and_ip(s: &str, chains: HashSet<Id>) -> Result<PeerMetadata> {
        let parts: Vec<&str> = s.split('@').collect();
        if parts.len() == 1 {
            let ip: SocketAddr = parts[0].parse().map_err(|_| Error::PeerParseError)?;
            let id = Id::from_ip(&ip);
            Ok(PeerMetadata { id, ip, chains })
        } else if parts.len() == 2 {
            let id: Id = parts[0].parse().map_err(|_| Error::PeerParseError)?;
            let ip: SocketAddr = parts[1].parse().map_err(|_| Error::PeerParseError)?;
            Ok(PeerMetadata { id, ip, chains })
        } else {
            Err(Error::PeerParseError)
        }
    }
}


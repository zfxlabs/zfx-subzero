use crate::p2p::id::Id;

use super::Result;
use crate::cell::types::{Capacity, PublicKeyHash};

use ed25519_dalek::Keypair;

use std::str::FromStr;

pub struct InitialStaker {
    pub keypair: Keypair,
    pub node_id: Id,
    pub total_allocation: Capacity,
    pub staked_allocation: Capacity,
}

impl InitialStaker {
    pub fn from_hex(
        kps: String,
        node_id: Id,
        total_allocation: Capacity,
        staked_allocation: Capacity,
    ) -> Result<Self> {
        let bytes = hex::decode(kps)?;
        let keypair = Keypair::from_bytes(&bytes)?;
        Ok(InitialStaker { keypair, node_id, total_allocation, staked_allocation })
    }

    pub fn public_key_hash(&self) -> Result<PublicKeyHash> {
        let encoded = bincode::serialize(&self.keypair.public)?;
        Ok(blake3::hash(&encoded).as_bytes().clone())
    }
}

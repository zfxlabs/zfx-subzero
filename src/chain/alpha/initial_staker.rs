use super::tx::{PublicKeyHash, Amount};

use zfx_id::Id;

use ed25519_dalek::Keypair;

pub struct InitialStaker {
    pub keypair: Keypair,
    pub node_id: Id,
    pub allocation: Amount,
    pub staked: Amount,
}

impl InitialStaker {
    pub fn new(keypair: Keypair, node_id: Id, allocation: Amount, staked: Amount) -> Self {
	InitialStaker { keypair, node_id, allocation, staked }
    }

    pub fn from_hex(kps: String, node_id: Id, allocation: Amount, staked: Amount) -> Self {
	let bytes = hex::decode(kps).unwrap();
	let keypair = Keypair::from_bytes(&bytes).unwrap();
	InitialStaker { keypair, node_id, allocation, staked }
    }

    pub fn public_key_hash(&self) -> PublicKeyHash {
	let encoded = bincode::serialize(&self.keypair.public).unwrap();
	blake3::hash(&encoded).as_bytes().clone()
    }
}

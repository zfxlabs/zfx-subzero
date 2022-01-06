use super::tx::{PublicKeyHash, Amount};

use zfx_id::Id;

use ed25519_dalek::Keypair;

pub struct InitialStaker {
    pub keypair: Keypair,
    pub node_id: Id,
    pub amount: Amount,
}

impl InitialStaker {
    pub fn new(keypair: Keypair, node_id: Id, amount: Amount) -> Self {
	InitialStaker { keypair, node_id, amount }
    }

    pub fn from_hex(s: String, node_id: Id, amount: Amount) -> Self {
	let bytes = hex::decode(s).unwrap();
	let keypair = Keypair::from_bytes(&bytes).unwrap();
	InitialStaker { keypair, node_id, amount }
    }

    pub fn public_key_hash(&self) -> PublicKeyHash {
	let encoded = bincode::serialize(&self.keypair.public).unwrap();
	blake3::hash(&encoded).as_bytes().clone()
    }
}

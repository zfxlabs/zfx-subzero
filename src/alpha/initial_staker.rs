use crate::zfx_id::Id;

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

pub fn genesis_stakers() -> Vec<InitialStaker> {
    vec![
	InitialStaker::from_hex(
		"ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416".to_owned(),
		Id::from_str("12My22AzQQosboCy6TCDFkTQwHTSuHhFN1VDcdDRPUe3H8j3DvY").unwrap(),
		2000, // 2000 allocated
		1000, // half of it staked so that we can transfer funds later
	    ).unwrap(),
	    InitialStaker::from_hex(
		"5a353c630d3faf8e2d333a0983c1c71d5e9b6aed8f4959578fbeb3d3f3172886393b576de0ac1fe86a4dd416cf032543ac1bd066eb82585f779f6ce21237c0cd".to_owned(),
		Id::from_str("19Y53ymnBw4LWUpiAMUzPYmYqZmukRhNHm3VyAhzMqckRcuvkf").unwrap(),
		2000,
		1000,
	    ).unwrap(),
	    InitialStaker::from_hex(
		"6f4b736b9a6894858a81696d9c96cbdacf3d49099d212213f5abce33da18716f067f8a2b9aeb602cd4163291ebbf39e0e024634f3be19bde4c490465d9095a6b".to_owned(),
		Id::from_str("1A2iUK1VQWMfvtmrBpXXkVJjM5eMWmTfMEcBx4TatSJeuoSH7n").unwrap(),
		2000,
		1000,
	    ).unwrap(),
        InitialStaker::from_hex(
        "3ae38eec96146c241f6cadf01995af14f027b23b8fecbc77dbc2e3ed5fec6fc3fb4fe5534f7affc9a8f1d99e290fdb91cc26777edd6fae480cad9f735d1b3680".to_owned(),
        Id::from_str("12StzamTJk2jBxbdqGmT6gLfpctv9f39CmBXTsm8sBG2n6AdPxx").unwrap(),
        2000,
        1000,
        ).unwrap(),
        InitialStaker::from_hex(
        "aae4e1343eb40e217a60fc61e22b86925686e664d7663c09d0042eb049600e187a2049a994e5b7a3e2baa9341c697029550ee0782d83ba31fe10fa0fefd6cc52".to_owned(),
        Id::from_str("1tJB1qNY6R4nPGQN83hmX8bviD6dbEMXkGjfByrCVYZsNnrJSk").unwrap(),
        2000,
        1000,
        ).unwrap(),
        InitialStaker::from_hex(
        "8c739c713aeb69e21a37bc2aab2ab314d08627d5435754b0418a71529c3614bccdfa638fa8da6d06e98a374c1df48e3a3d2563a4c7d78d0e7589f6706a8ed0d8".to_owned(),
        Id::from_str("12KyV3nz5wJhqFSfEFsKAhEqMGaPD88JeeS7LA4Qsjbyf2Yqp87").unwrap(),
        2000,
        1000,
        ).unwrap(),
    ]
}

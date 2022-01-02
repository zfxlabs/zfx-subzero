use super::tx::StakeTx;

use tai64::Tai64;

use crate::util;

use std::net::SocketAddr;

pub type BlockHash = [u8; 32];
pub type VrfOutput = [u8; 32];
pub type Height = u64;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Block {
    pub predecessor: Option<BlockHash>,
    pub height: Height,
    pub vrf_out: VrfOutput,
    pub txs: Vec<StakeTx>,
}

pub fn genesis_vrf_out() -> [u8; 32] {
    let mut vrf_out = [0u8; 32];
    let vrf_out_v = hex::decode("57e1e774e97685b9dc2dbcb7a327fa96a60dcda0919ad1b75877885bd219bfc4").unwrap();
    for i in 0..32 {
	vrf_out[i] = vrf_out_v[i];
    }
    vrf_out
}

pub fn genesis() -> Block {
    Block {
	predecessor: None,
	height: 0u64,
	vrf_out: genesis_vrf_out(),
	txs: vec![
	    StakeTx::new(
		util::id_from_ip(&"127.0.0.1:1234".parse().unwrap()),
		// start_time.clone(),
		// start_time.clone() + 3 * 60,
		1000u64,
	    ),
	    StakeTx::new(
		util::id_from_ip(&"127.0.0.1:1235".parse().unwrap()),
		// start_time.clone(),
		// start_time.clone() + 6 * 60,
		1000u64,
	    ),
	    StakeTx::new(
		util::id_from_ip(&"127.0.0.1:1236".parse().unwrap()),
		// start_time.clone(),
		// start_time.clone() + 9 * 60,
		1000u64,
	    ),
	],
    }
}

impl Block {
    pub fn new(predecessor: BlockHash, height: u64, vrf_out: VrfOutput, txs: Vec<StakeTx>) -> Block {
	Block {
	    predecessor: Some(predecessor),
	    height,
	    vrf_out,
	    txs,
	}
    }

    // FIXME: Assumption: blake3 produces a big-endian hash
    pub fn hash(&self) -> BlockHash {
	let encoded = bincode::serialize(self).unwrap();
	blake3::hash(&encoded).as_bytes().clone()
    }
}


use crate::chain::alpha::InitialStaker;
use crate::chain::alpha::tx::Transaction;
use crate::chain::alpha::tx::{CoinbaseTx, StakeTx};
use crate::util;

use tai64::Tai64;

use std::net::SocketAddr;

pub type BlockHash = [u8; 32];
pub type VrfOutput = [u8; 32];
pub type Height = u64;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Block {
    pub predecessor: Option<BlockHash>,
    pub height: Height,
    pub vrf_out: VrfOutput,
    pub txs: Vec<Transaction>,
}

pub fn genesis_vrf_out() -> [u8; 32] {
    let mut vrf_out = [0u8; 32];
    let vrf_out_v = hex::decode("57e1e774e97685b9dc2dbcb7a327fa96a60dcda0919ad1b75877885bd219bfc4").unwrap();
    for i in 0..32 {
	vrf_out[i] = vrf_out_v[i];
    }
    vrf_out
}

pub fn genesis(initial_stakers: Vec<InitialStaker>) -> Block {
    let mut txs = vec![];
    for staker in initial_stakers.iter() {
	let pkh = staker.public_key_hash();
	let alloc_tx = CoinbaseTx::new(pkh, staker.amount.clone());
	let stake_tx = StakeTx::new(
	    &staker.keypair,
	    staker.node_id.clone(),
	    alloc_tx.hash(),
	    0,
	    staker.amount.clone(),
	);
	txs.push(Transaction::CoinbaseTx(alloc_tx.clone()));
	txs.push(Transaction::StakeTx(stake_tx.clone()));
    }
    Block {
	predecessor: None,
	height: 0u64,
	vrf_out: genesis_vrf_out(),
	txs,
    }
}

impl Block {
    pub fn new(predecessor: BlockHash, height: u64, vrf_out: VrfOutput, txs: Vec<Transaction>) -> Block {
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

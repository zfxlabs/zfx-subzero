use super::coinbase::CoinbaseOperation;
use super::initial_staker::genesis_stakers;
use super::stake::StakeOperation;
use super::Result;
use crate::cell::Cell;

use std::convert::TryInto;

pub type BlockHash = [u8; 32];
pub type BlockHeight = u64;
pub type VrfOutput = [u8; 32];

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Block {
    pub predecessor: Option<BlockHash>,
    pub height: BlockHeight,
    pub vrf_out: VrfOutput,
    pub cells: Vec<Cell>,
}

/// The genesis VRF output - a random set of bytes.
pub fn genesis_vrf_out() -> Result<[u8; 32]> {
    let mut vrf_out = [0u8; 32];
    let vrf_out_v =
        hex::decode("57e1e774e97685b9dc2dbcb7a327fa96a60dcda0919ad1b75877885bd219bfc4")?;
    for i in 0..32 {
        vrf_out[i] = vrf_out_v[i];
    }
    Ok(vrf_out)
}

pub fn build_genesis() -> Result<Block> {
    let initial_stakers = genesis_stakers();
    // Aggregate the allocations into one coinbase output so that the conflict graph has one genesis
    // vertex.
    let mut allocations = vec![];
    for staker in initial_stakers.iter() {
        let pkh = staker.public_key_hash()?;
        allocations.push((pkh.clone(), staker.total_allocation.clone()));
    }
    let allocations_op = CoinbaseOperation::new(allocations);
    let allocations_tx: Cell = allocations_op.try_into()?;
    // Construct the genesis block.
    let mut cells = vec![];
    for staker in initial_stakers.iter() {
        let pkh = staker.public_key_hash()?;
        let stake_op = StakeOperation::new(
            allocations_tx.clone(),
            pkh.clone(),
            staker.staked_allocation.clone(),
        );
        let stake_tx = stake_op.stake(&staker.keypair)?;
        cells.push(stake_tx);
    }
    cells.push(allocations_tx);
    Ok(Block { predecessor: None, height: 0u64, vrf_out: genesis_vrf_out()?, cells })
}

impl Block {
    pub fn new(predecessor: BlockHash, height: u64, vrf_out: VrfOutput, cells: Vec<Cell>) -> Block {
        Block { predecessor: Some(predecessor), height, vrf_out, cells }
    }

    // FIXME: Assumption: blake3 produces a big-endian hash
    pub fn hash(&self) -> Result<BlockHash> {
        let encoded = bincode::serialize(self)?;
        Ok(blake3::hash(&encoded).as_bytes().clone())
    }
}

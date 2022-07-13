use super::coinbase::CoinbaseOperation;
use super::constants;
use super::initial_staker::genesis_stakers;
use super::stake::StakeOperation;
use super::types::{BlockHash, BlockHeight, VrfOutput};
use super::Result;
use crate::cell::Cell;
use crate::util;

use std::convert::TryInto;

/// Data structure for storing block-related information
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Block {
    /// Previous block, linked to this one
    pub predecessor: Option<BlockHash>,
    /// Height of the block
    pub height: BlockHeight,
    /// Proof of validity of the block
    pub vrf_out: VrfOutput,
    /// A list of [Cell]s of this block
    pub cells: Vec<Cell>,
}

impl std::fmt::Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut s = match self.predecessor {
            Some(predecessor) => format!("predecessor = {}\n", hex::encode(predecessor)),
            None => format!("predecessor = None\n"),
        };
        s = format!("{}block_height = {:?}\n", s, self.height);
        s = format!("{}vrf_output = {}", s, hex::encode(self.vrf_out));
        write!(f, "{}\n", s)
    }
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

/// Create a genesis block with [Cell]s from the [initial stakers](crate::alpha::initial_staker::genesis_stakers).
pub fn build_genesis() -> Result<Block> {
    let initial_stakers = genesis_stakers();
    // Aggregate the allocations into one coinbase output so that the conflict graph has one genesis
    // vertex.
    let mut allocations = vec![];
    let current_time = util::get_utc_timestamp_millis();
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
            staker.node_id.clone(),
            pkh.clone(),
            staker.staked_allocation.clone(),
            current_time,
            current_time + constants::STAKING_DURATION,
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

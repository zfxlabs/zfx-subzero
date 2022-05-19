use super::block::Block;
use super::block_header::BlockHeader;
use super::coinbase::CoinbaseOperation;
use super::initial_staker::InitialStaker;
use super::stake::StakeOperation;
use super::Result;

use crate::cell::Cell;
use crate::zfx_id::Id;

use std::convert::TryInto;
use std::str::FromStr;

/// Builds the genesis block for the primary network (alpha protocol).
pub fn build_genesis_block() -> Result<Block> {
    let initial_stakers = initial_stakers();

    // Aggregate the allocations into one coinbase output so that the conflict graph has
    // one genesis vertex
    let mut allocations = vec![];
    for staker in initial_stakers.iter() {
        let pkh = staker.public_key_hash()?;
        allocations.push((pkh.clone(), staker.total_allocation.clone()));
    }
    let allocations_op = CoinbaseOperation::new(allocations);
    let allocations_tx: Cell = allocations_op.try_into()?;

    // Construct the genesis block
    let mut cells = vec![];
    for staker in initial_stakers.iter() {
        let pkh = staker.public_key_hash()?;
        let stake_op = StakeOperation::new(
            allocations_tx.clone(),
            staker.node_id.clone(),
            pkh.clone(),
            staker.staked_allocation.clone(),
        );
        let stake_tx = stake_op.stake(&staker.keypair)?;
        cells.push(stake_tx);
    }
    cells.push(allocations_tx);

    // TODO: Generate a merkle root from the transactions (..)

    let header = BlockHeader::new(0u64, None, genesis_vrf_output()?);
    Ok(Block { header, cells })
}

/// Builds the genesis list of initial stakers for the primary network (alpha protocol).
fn initial_stakers() -> Vec<InitialStaker> {
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
    ]
}

/// The pre-agreed genesis VRF output - a random set of bytes.
fn genesis_vrf_output() -> Result<[u8; 32]> {
    let mut vrf_output = [0u8; 32];
    let vrf_output_v =
        hex::decode("57e1e774e97685b9dc2dbcb7a327fa96a60dcda0919ad1b75877885bd219bfc4")?;
    for i in 0..32 {
        vrf_output[i] = vrf_output_v[i];
    }
    Ok(vrf_output)
}

#[cfg(test)]
mod test {
    #[actix_rt::test]
    async fn test_build_genesis() {
        let _block = build_genesis_block().unwrap();
    }
}

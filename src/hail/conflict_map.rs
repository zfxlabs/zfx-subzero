use super::{Error, Result};

use super::conflict_set::ConflictSet;
use super::vertex::Vertex;

use crate::alpha::block::Block;
use crate::alpha::types::{BlockHash, BlockHeight};

use super::hail::BETA1;

use std::collections::{hash_map::Entry, HashMap, HashSet};

use tracing::info;

pub struct ConflictMap {
    inner: HashMap<BlockHeight, ConflictSet>,
}

impl ConflictMap {
    pub fn new() -> Self {
        ConflictMap { inner: HashMap::default() }
    }

    /// Whether the block designated by `BlockHash` is preferred within its conflicting set.
    pub fn is_preferred(&self, height: &BlockHeight, block_hash: BlockHash) -> Result<bool> {
        match self.inner.get(height) {
            Some(cs) => Ok(cs.is_preferred(block_hash)),
            None => Err(Error::InvalidBlockHeight(height.clone())),
        }
    }

    /// Whether the block at `BlockHeight` has no other conflicts.
    pub fn is_singleton(&self, height: &BlockHeight) -> Result<bool> {
        match self.inner.get(height) {
            Some(cs) => Ok(cs.is_singleton()),
            None => Err(Error::InvalidBlockHeight(height.clone())),
        }
    }

    /// Fetch the preferred transaction.
    pub fn get_preferred(&self, height: &BlockHeight) -> Result<BlockHash> {
        match self.inner.get(height) {
            Some(cs) => Ok(cs.pref),
            None => Err(Error::InvalidBlockHeight(height.clone())),
        }
    }

    pub fn get_confidence(&self, vx: &Vertex) -> Result<u8> {
        match self.inner.get(&vx.height) {
            Some(cs) => {
                if cs.pref == vx.block_hash {
                    Ok(cs.cnt)
                } else {
                    Ok(0)
                }
            }
            None => Err(Error::InvalidBlockHeight(vx.height.clone())),
        }
    }

    pub fn insert_block(&mut self, block: Block) -> Result<ConflictSet> {
        match self.inner.entry(block.height.clone()) {
            // The conflict set already contains a conflict.
            Entry::Occupied(mut o) => {
                let cs = o.get_mut();
                let block_hash = block.hash()?;
                cs.conflicts.insert(block_hash.clone());
                // If the confidence is still 0 and the lowest hash in the set is this block hash,
                // then prefer this block.
                if cs.cnt < BETA1 && cs.is_lowest_hash(block_hash.clone()) {
                    info!(
                        "[conflicts] !! block = {} supersedes {}",
                        hex::encode(block_hash.clone()),
                        hex::encode(cs.pref),
                    );
                    cs.pref = block_hash;
                    cs.cnt = 0;
                }
                Ok(cs.clone())
            }
            // The block is currently non-conflicting.
            Entry::Vacant(v) => {
                let mut cs = ConflictSet::new(block.hash().unwrap());
                v.insert(cs.clone());
                Ok(cs)
            }
        }
    }

    pub fn update_conflict_set(
        &mut self,
        height: BlockHeight,
        block_hash: BlockHash,
        d1: u8,
        d2: u8,
    ) -> Result<()> {
        match self.inner.entry(height.clone()) {
            Entry::Occupied(mut o) => {
                let cs = o.get_mut();
                if d1 > d2 {
                    cs.pref = block_hash.clone();
                }
                if block_hash != cs.last {
                    cs.last = block_hash.clone();
                } else {
                    cs.cnt += 1;
                }
                Ok(())
            }
            Entry::Vacant(_) => Err(Error::InvalidBlockHeight(height.clone())),
        }
    }
}

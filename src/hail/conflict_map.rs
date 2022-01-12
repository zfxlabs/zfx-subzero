use super::{Error, Result};

use super::conflict_set::ConflictSet;

use crate::chain::alpha::block::{Block, BlockHash, Height};

use std::collections::{hash_map::Entry, HashMap, HashSet};

pub struct ConflictMap {
    inner: HashMap<Height, ConflictSet<BlockHash>>,
}

impl ConflictMap {
    pub fn new() -> Self {
        ConflictMap { inner: HashMap::default() }
    }

    /// Whether the block designated by `BlockHash` is preferred within its conflicting set.
    pub fn is_preferred(&self, height: &Height, block_hash: BlockHash) -> Result<bool> {
        match self.inner.get(height) {
            Some(cs) => Ok(cs.is_preferred(block_hash)),
            None => Err(Error::InvalidHeight(height.clone())),
        }
    }

    /// Whether the block at `Height` has no other conflicts.
    pub fn is_singleton(&self, height: &Height) -> Result<bool> {
        match self.inner.get(height) {
            Some(cs) => Ok(cs.is_singleton()),
            None => Err(Error::InvalidHeight(height.clone())),
        }
    }

    /// Fetch the preferred transaction.
    pub fn get_preferred(&self, height: &Height) -> Result<BlockHash> {
        match self.inner.get(height) {
            Some(cs) => Ok(cs.pref),
            None => Err(Error::InvalidHeight(height.clone())),
        }
    }

    pub fn get_confidence(&self, height: &Height) -> Result<u8> {
        match self.inner.get(height) {
            Some(cs) => Ok(cs.cnt),
            None => Err(Error::InvalidHeight(height.clone())),
        }
    }

    pub fn insert_block(&mut self, block: Block) -> ConflictSet<BlockHash> {
        match self.inner.entry(block.height.clone()) {
            // The conflict set already contains a conflict.
            Entry::Occupied(mut o) => {
                let cs = o.get_mut();
                cs.conflicts.insert(block.hash());
                // If the confidence is still 0 and the lowest hash in the set is this block hash,
                // then prefer this block.
                if cs.cnt == 0 {
                    // && cs.lowest_hash() == block.hash() {
                    cs.pref = block.hash();
                }
                cs.clone()
            }
            // The block is currently non-conflicting.
            Entry::Vacant(v) => {
                let mut cs = ConflictSet::new(block.hash());
                v.insert(cs.clone());
                cs
            }
        }
    }

    pub fn update_conflict_set(
        &mut self,
        height: Height,
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
            Entry::Vacant(_) => Err(Error::InvalidHeight(height.clone())),
        }
    }
}

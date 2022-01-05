use crate::chain::alpha::block::{Block, BlockHash, Height};

use std::collections::{HashSet, HashMap, hash_map::Entry};

pub struct HeightMap {
    inner: HashMap<Height, HashSet<BlockHash>>,
}

impl HeightMap {
    /// Creates a new height map containing a mapping of height to a hashset of potentially
    /// conflicting blocks for that height.
    pub fn new() -> Self {
	HeightMap { inner: HashMap::default() }
    }

    /// Inserts the height referenced in the supplied block.
    pub fn insert_block(&mut self, block: Block) {
	match self.inner.entry(block.height.clone()) {
	    Entry::Occupied(mut o) => {
		// The height already exists, insert the hash of the conflicting block.
		let block_hashes = o.get_mut();
		block_hashes.insert(block.hash());
	    },
	    Entry::Vacant(mut v) => {
		// The height has no existing conflicts, insert as a singleton set.
		let mut hs = HashSet::new();
		hs.insert(block.hash());
		v.insert(hs);
	    },
	}
    }

    /// Returns the hashes of all blocks which conflict with the supplied block.
    pub fn conflicting_blocks(&self, block: Block) -> HashSet<BlockHash> {
	let mut hs: HashSet<BlockHash> = HashSet::new();
	match self.inner.get(&block.height) {
	    Some(block_hashes) => {
		for block_hash in block_hashes.iter().cloned() {
		    hs.insert(block_hash);
		}
	    },
	    None => (),
	}
	hs
    }
}

use crate::chain::alpha::block::{Block, BlockHash};

/// A conflict set references blocks which conflict at a given height and specifies which
/// block within the set is currently *preferred*, the *last* block which was assessed and
/// a *count* denoting the level of confidence accrued for this particular block.
pub struct ConflictSet {
    pub conflicts: Vec<Block>,
    pub pref: Block, // TODO: use the block hash
    pub last: Block,      // TODO: use the block hash
    pub cnt: u8,
}

impl ConflictSet {
    
    // TODO: use the block hash
    pub fn new(preferred: Block) -> Self {
        ConflictSet {
            conflicts: vec![preferred.clone()],
            last: preferred.clone(),
            pref: preferred,
            cnt: 0u8,
	}
    }

    pub fn insert(&mut self, block: Block) {
	self.conflicts.push(block)
    }

    pub fn lowest_hash(&self) -> BlockHash {
	let mut h = self.conflicts[0].hash();
	for i in 1..self.conflicts.len() {
	    let hi = self.conflicts[i].hash();
	    if hi < h {
		h = hi;
	    }
	}
	h
    }

}

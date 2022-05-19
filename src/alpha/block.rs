use super::block_header::BlockHeader;
use super::types::{BlockHash, BlockHeight, VrfOutput};
use super::Result;

use crate::cell::Cell;

use std::convert::TryInto;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub cells: Vec<Cell>,
}

impl std::fmt::Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.header.fmt(f)
    }
}

impl Block {
    pub fn new(header: BlockHeader, cells: Vec<Cell>) -> Block {
        Block { header, cells }
    }

    pub fn height(&self) -> BlockHeight {
        self.header.height()
    }

    pub fn predecessor(&self) -> Option<BlockHash> {
        self.header.predecessor()
    }

    pub fn vrf_output(&self) -> VrfOutput {
        self.header.vrf_output()
    }

    // FIXME: Assumption: blake3 produces a big-endian hash
    pub fn hash(&self) -> Result<BlockHash> {
        let encoded = bincode::serialize(self)?;
        Ok(blake3::hash(&encoded).as_bytes().clone())
    }
}

use super::vertex::Vertex;
use super::{Error, Result};
use crate::alpha::block::Block;
use crate::alpha::types::{BlockHash, BlockHeight};

use crate::colored::Colorize;

/// The `HailBlock` is a consensus specific representation of a block which contains a real block.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct HailBlock {
    parent: Vertex,
    block: Block,
}

impl HailBlock {
    pub fn new(parent: Vertex, block: Block) -> Self {
        HailBlock { parent, block }
    }

    pub fn parent(&self) -> Vertex {
        self.parent.clone()
    }

    pub fn height(&self) -> BlockHeight {
        self.block.height.clone()
    }

    pub fn vertex(&self) -> Result<Vertex> {
        Ok(Vertex::new(self.height(), self.hash()?))
    }

    pub fn inner(&self) -> Block {
        self.block.clone()
    }

    /// Returns the hash of the inner block.
    pub fn hash(&self) -> Result<BlockHash> {
        self.block.hash().map_err(|err| Error::Alpha(err))
    }
}

impl std::fmt::Display for HailBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = format!("{}", self.block);
        let mut ps = "".to_owned();
        let h = hex::encode(self.parent.block_hash);
        ps.push(' ');
        ps.push_str(&h);
        let s = format!("{}[{}]{}\n", s, "parent".yellow(), ps);
        write!(f, "{}", s)
    }
}

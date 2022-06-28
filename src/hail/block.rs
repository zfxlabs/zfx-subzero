//! [HailBlock] is a consensus specific representation of a block
use super::vertex::Vertex;
use super::{Error, Result};
use crate::alpha::block::Block;
use crate::alpha::types::{BlockHash, BlockHeight, VrfOutput};

use crate::colored::Colorize;

/// The `HailBlock` is a consensus specific representation of a block which contains a real block
/// along with a parent vertex which points to its predecessor (must be height - 1).
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct HailBlock {
    /// The parent vertex of this block (genesis is `None`, all other blocks have a parent).
    parent: Option<Vertex>,
    /// The inner block contents.
    block: Block,
}

impl HailBlock {
    /// Create a new block
    pub fn new(parent: Option<Vertex>, block: Block) -> Self {
        HailBlock { parent, block }
    }

    /// Returns the parent vertex of this block if this block is not genesis.
    pub fn parent(&self) -> Option<Vertex> {
        self.parent.clone()
    }

    /// Returns the height of the contained block.
    pub fn height(&self) -> BlockHeight {
        self.block.height.clone()
    }

    /// Return the VRF output of the inner `Block`
    pub fn vrf_output(&self) -> VrfOutput {
        self.block.vrf_out.clone()
    }

    /// Returns a vertex formed from the height and the hash of the block.
    pub fn vertex(&self) -> Result<Vertex> {
        Ok(Vertex::new(self.height(), self.hash()?))
    }

    /// Returns the inner block.
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
        match &self.parent {
            Some(parent) => {
                let h = hex::encode(parent.block_hash);
                ps.push(' ');
                ps.push_str(&h);
            }
            None => (),
        };
        let s = format!("{}[{}]{}\n", s, "parent".yellow(), ps);
        write!(f, "{}", s)
    }
}

use super::{Error, Result};
use crate::alpha::block::Block;
use crate::alpha::types::BlockHash;

/// The `HailBlock` is a consensus specific representation of a block which contains a real block.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct HailBlock {
    pub parents: Vec<BlockHash>,
    pub block: Block,
}

impl HailBlock {
    pub fn new(parents: Vec<BlockHash>, block: Block) -> Self {
        HailBlock { parents, block }
    }

    /// Returns the hash of the inner block.
    pub fn hash(&self) -> Result<BlockHash> {
        self.block.hash().map_err(|err| Error::Alpha(err))
    }
}

// impl std::fmt::Display for HailBlock {
//     fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//         let s = format!("{}", self.cell);
//         let mut ps = "".to_owned();
//         for p in self.parents.iter() {
//             let h = hex::encode(p);
//             ps.push(' ');
//             ps.push_str(&h);
//         }
//         let s = format!("{}[{}]{}\n", s, "parents".yellow(), ps);
//         write!(f, "{}", s)
//     }
// }

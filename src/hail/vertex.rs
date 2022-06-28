use crate::alpha::types::{BlockHash, BlockHeight};

/// Vertex of the [Hail][super::Hail] graph
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Vertex {
    pub height: BlockHeight,
    pub block_hash: BlockHash,
}

impl Vertex {
    /// Creates a new [Vertex]
    pub fn new(height: BlockHeight, block_hash: BlockHash) -> Self {
        Vertex { height, block_hash }
    }
}

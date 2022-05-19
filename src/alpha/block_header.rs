use super::types::{BlockHash, BlockHeight, VrfOutput};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct BlockHeader {
    /// The current block height
    height: BlockHeight,
    /// Must be able to prove that a block extends from another
    predecessor: Option<BlockHash>,
    // Must be able to prove that transactions were included in the block
    // pub data_root: MerkleRoot,
    // The number of leaves in the data root
    // pub data_length: u64,
    // Must be able to prove that this block created a valid state transition
    // pub state_root
    /// Must be able to prove that a validator had the right to produce the block
    vrf_output: VrfOutput,
}

impl std::fmt::Display for BlockHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut s = match self.predecessor {
            Some(predecessor) => format!("predecessor = {}\n", hex::encode(predecessor)),
            None => format!("predecessor = None\n"),
        };
        s = format!("{}block_height = {:?}\n", s, self.height);
        s = format!("{}vrf_output = {}", s, hex::encode(self.vrf_output));
        write!(f, "{}\n", s)
    }
}

impl BlockHeader {
    pub fn new(
        height: BlockHeight,
        predecessor: Option<BlockHash>,
        vrf_output: VrfOutput,
    ) -> BlockHeader {
        BlockHeader { height, predecessor, vrf_output }
    }

    pub fn height(&self) -> BlockHeight {
        self.height
    }

    pub fn predecessor(&self) -> Option<BlockHash> {
        self.predecessor
    }

    pub fn vrf_output(&self) -> VrfOutput {
        self.vrf_output
    }
}

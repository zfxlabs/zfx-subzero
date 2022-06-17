use crate::alpha::types::BlockHash;
use crate::hail::block::HailBlock;
use crate::p2p::id::Id;

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "QueryBlockAck")]
pub struct QueryBlock {
    pub id: Id,
    pub block: HailBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct QueryBlockAck {
    pub id: Id,
    pub block_hash: BlockHash,
    pub outcome: bool,
}

use crate::chain::alpha::block::{Block, Height, VrfOutput};
use crate::chain::alpha::tx::StakeTx;

use super::conflict_set::ConflictSet;

use tracing::{info, debug};

use actix::{Actor, Context, Handler, Addr};

use std::collections::{HashMap, hash_map::Entry};

struct Consensus {
    /// The set of all live blocks (non-final)
    live_blocks: Vec<Block>,
    /// The set of all queried blocks
    queried_blocks: Vec<Block>,
    /// The map of conflicting blocks at a particular height
    conflict_map: HashMap<Height, ConflictSet>,
}

impl Consensus {
    /// Consensus is initialised with the most recent `frontier`, which is the last set of
    /// blocks yet to become final.
    pub fn new(frontier: Vec<Block>) -> Self {
	Consensus {
	    live_blocks: frontier,
	    queried_blocks: vec![],
	    conflict_map: HashMap::default(),
	}
    }

    /// Parent selection selects the most preferred block found within the conflict set at
    /// a given height of `h - 1` with respect to the block being proposed.
    pub fn select_parent(&mut self, height: Height) -> Block {
	// If the conflict map is empty then consensus is now being started, thus we must
	// recover the conflict map from the latest frontier.
	
	// Fetch the preferred entry at the provided height.
	if let Entry::Occupied(o) = self.conflict_map.entry(height) {
	    let cs: &ConflictSet = o.get();
	    cs.pref.clone()
	} else {
	    panic!("non-continuous height within consensus : erroneous bootstrap");
	}
    }
}

impl Actor for Consensus {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
	debug!(": started");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "QueryBlockAck")]
pub struct QueryBlock;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct QueryBlockAck;

impl Handler<QueryBlock> for Consensus {
    type Result = QueryBlockAck;

    fn handle(&mut self, msg: QueryBlock, _ctx: &mut Context<Self>) -> Self::Result {
	QueryBlockAck {}
    }
}

// Generate a block whenever we are a block producer and have pending transactions in
// `sleet`.
// pub async fn generate_block() {}

// pub async fn run() { }
    

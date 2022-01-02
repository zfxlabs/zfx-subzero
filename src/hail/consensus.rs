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
#[rtype(result = "()")]
pub struct ReceiveBlock {
    block: Block,
}

impl Handler<ReceiveBlock> for Consensus {
    type Result = ();

    fn handle(&mut self, msg: ReceiveBlock, _ctx: &mut Context<Self>) -> Self::Result {
	let block = msg.block.clone();

	// Check that the block does not already exist
	// state::exists_block(block_hash) -> bool

	// Insert the block into the conflict map or create a new entry
	match self.conflict_map.entry(block.height.clone()) {
	    Entry::Occupied(mut o) => {
		let cs = o.get_mut();
		let () = cs.insert(block.clone());
		let h = block.clone().hash();
		if cs.cnt == 0 && h == cs.lowest_hash() {
		    cs.pref = block.clone();
		    cs.last = block.clone();
		}
	    },
	    Entry::Vacant(v) => {
		let _old = v.insert(ConflictSet::new(block.clone()));
		()
	    },
	}

	// Persist the block with a chit of 0
	// state::put_block(block.clone(), 0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct GenerateBlock {
    height: Height,
    vrf_out: VrfOutput,
    txs: Vec<StakeTx>,
}

impl Handler<GenerateBlock> for Consensus {
    type Result = ();

    fn handle(&mut self, msg: GenerateBlock, _ctx: &mut Context<Self>) -> Self::Result {
	let parent = self.select_parent(msg.height - 1);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "QueryResult")]
pub struct Query;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct QueryResult;

impl Handler<Query> for Consensus {
    type Result = QueryResult;

    fn handle(&mut self, msg: Query, _ctx: &mut Context<Self>) -> Self::Result {
	QueryResult {}
    }
}

pub async fn run() { }
    

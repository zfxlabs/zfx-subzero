use zfx_id::Id;
use zfx_sortition::sortition;

use crate::colored::Colorize;
use crate::chain::alpha::block::{Block, Height, VrfOutput};
use crate::chain::alpha::tx::StakeTx;
use crate::util;

use super::conflict_set::ConflictSet;

use tracing::{info, debug};

use actix::{Actor, Context, Handler, Addr};

use std::collections::{HashSet, HashMap, hash_map::Entry};

pub struct Hail {
    /// The set of all live blocks (non-final)
    live_blocks: Vec<Block>,
    /// The set of all queried blocks
    queried_blocks: Vec<Block>,
    /// The map of conflicting blocks at a particular height
    conflict_map: HashMap<Height, ConflictSet>,
}

impl Hail {
    /// Hail is initialised with the most recent `frontier`, which is the last set of
    /// blocks yet to become final.
    pub fn new(frontier: Vec<Block>) -> Self {
	Hail {
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

impl Actor for Hail {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
	debug!(": started");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct LiveCommittee {
    pub self_id: Id,
    pub height: u64,
    pub initial_supply: u64,
    pub validators: Vec<(Id, u64)>,
    pub vrf_out: VrfOutput,
}

fn compute_vrf_h(id: Id, vrf_out: &VrfOutput) -> [u8; 32] {
    let vrf_h = vec![id.as_bytes(), vrf_out].concat();
    blake3::hash(&vrf_h).as_bytes().clone()
}

impl Handler<LiveCommittee> for Hail {
    type Result = ();

    fn handle(&mut self, msg: LiveCommittee, _ctx: &mut Context<Self>) -> Self::Result {
	info!("{} received live committee at height = {:?}", "[hail]".blue(), msg.height);
	let self_id = msg.self_id.clone();
	let expected_size = (msg.validators.len() as f64).sqrt().ceil();
	info!("expected_size = {:?}", expected_size);

	let mut validators = vec![];
	let mut block_producers = HashSet::new();
	let mut block_production_slot = None;
	for (id, qty) in msg.validators {
	    let vrf_h = compute_vrf_h(id.clone(), &msg.vrf_out);
	    let s_w = sortition::select(qty, msg.initial_supply, expected_size, &vrf_h);
	    // If the sortition weight > 0 then this `id` is a block producer.
	    if s_w > 0 {
		block_producers.insert(id.clone());
	    }
	    // If the sortition weight > 0 and this is our `id`, we have a slot to produce
	    // the next block.
	    if s_w > 0 && id.clone() == self_id {
		block_production_slot = Some(vrf_h.clone());
	    }
	    let v_w = util::percent_of(qty, msg.initial_supply);
	    validators.push((id.clone(), v_w));
	}

	// If we are the next block producer, ...
	info!("is_block_producer = {:?}", block_production_slot.is_some());

	// Otherwise wait for the next block to be received
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "QueryBlockAck")]
pub struct QueryBlock;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct QueryBlockAck;

impl Handler<QueryBlock> for Hail {
    type Result = QueryBlockAck;

    fn handle(&mut self, msg: QueryBlock, _ctx: &mut Context<Self>) -> Self::Result {
	QueryBlockAck {}
    }
}

// Generate a block whenever we are a block producer and have pending transactions in
// `sleet`.
// pub async fn generate_block() {}

// pub async fn run() { }

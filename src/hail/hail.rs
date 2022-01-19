use crate::zfx_id::Id;
use zfx_sortition::sortition;

use crate::cell::Cell;
use crate::chain::alpha::block::{Block, BlockHash, Height, VrfOutput};
use crate::chain::alpha::state::Weight;
use crate::client::Fanout;
use crate::colored::Colorize;
use crate::graph::DAG;
use crate::protocol::{Request, Response};
use crate::util;

use super::conflict_map::ConflictMap;
use super::conflict_set::ConflictSet;
use super::{Error, Result};

use tracing::{debug, info};

use actix::{Actor, AsyncContext, Context, Handler, Recipient, ResponseFuture};
use actix::{ActorFutureExt, ResponseActFuture, WrapFuture};

use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::net::SocketAddr;

// Safety parameters

const ALPHA: f64 = 0.5;
const BETA1: u8 = 11;
const BETA2: u8 = 20;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Vertex {
    height: Height,
    block_hash: BlockHash,
}

impl Vertex {
    pub fn new(height: Height, block_hash: BlockHash) -> Self {
        Vertex { height, block_hash }
    }
}

/// Hail is a Snow* based consensus for blocks.
pub struct Hail {
    /// The client used to make external requests.
    sender: Recipient<Fanout>,
    /// The identity of this validator.
    node_id: Id,
    /// The weighted validator set.
    committee: HashMap<Id, (SocketAddr, Weight)>,
    /// The set of all known blocks.
    known_blocks: sled::Db,
    /// The set of all queried blocks.
    queried_blocks: sled::Db,
    /// The map of conflicting blocks at a particular height
    conflict_map: ConflictMap,
    /// The consensus graph.
    dag: DAG<Vertex>,
}

impl Hail {
    /// Hail is initialised with the most recent `frontier`, which is the last set of
    /// blocks yet to become final.
    pub fn new(sender: Recipient<Fanout>, node_id: Id) -> Self {
        Hail {
            sender,
            node_id,
            committee: HashMap::default(),
            known_blocks: sled::Config::new().temporary(true).open().unwrap(),
            queried_blocks: sled::Config::new().temporary(true).open().unwrap(),
            conflict_map: ConflictMap::new(),
            dag: DAG::new(),
        }
    }

    // Branch preference

    /// Starts at some vertex and does a depth first search in order to compute whether
    /// the vertex is strongly preferred (by checking whether all its ancestry is
    /// preferred).
    pub fn is_strongly_preferred(&self, vx: Vertex) -> Result<bool> {
        for ancestor in self.dag.dfs(&vx) {
            if !self.conflict_map.is_preferred(&ancestor.height, ancestor.block_hash)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    // Adaptive Parent Selection

    /// Starts at the live edges (the leaf nodes) of the `DAG` and does a depth first
    /// search until a preferrential parent is found.
    pub fn select_parent(&mut self) -> Result<Option<Vertex>> {
        if self.dag.is_empty() {
            return Ok(None);
        }
        let leaves = self.dag.leaves();
        for leaf in leaves {
            for elt in self.dag.dfs(&leaf) {
                if self.is_strongly_preferred(elt.clone())? {
                    return Ok(Some(elt.clone()));
                }
            }
        }
        Ok(None)
    }

    pub fn sample(&self, minimum_weight: Weight) -> Result<Vec<(Id, SocketAddr)>> {
        let mut validators = vec![];
        for (id, (ip, w)) in self.committee.iter() {
            validators.push((id.clone(), ip.clone(), w.clone()));
        }
        util::sample_weighted(minimum_weight, validators).ok_or(Error::InsufficientWeight)
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
    pub total_stake: u64,
    pub validators: HashMap<Id, (SocketAddr, u64)>,
    pub vrf_out: VrfOutput,
}

fn compute_vrf_h(id: Id, vrf_out: &VrfOutput) -> [u8; 32] {
    let vrf_h = vec![id.as_bytes(), vrf_out].concat();
    blake3::hash(&vrf_h).as_bytes().clone()
}

impl Handler<LiveCommittee> for Hail {
    type Result = ();

    fn handle(&mut self, msg: LiveCommittee, _ctx: &mut Context<Self>) -> Self::Result {
        info!("[{}] received live committee at height = {:?}", "hail".blue(), msg.height);
        let self_id = msg.self_id.clone();
        let expected_size = (msg.validators.len() as f64).sqrt().ceil();
        info!("[{}] expected_size = {:?}", "hail".blue(), expected_size);

        let mut validators = vec![];
        let mut block_producers = HashSet::new();
        let mut block_production_slot = None;
        for (id, (_, qty)) in msg.validators {
            let vrf_h = compute_vrf_h(id.clone(), &msg.vrf_out);
            let s_w = sortition::select(qty, msg.total_stake, expected_size, &vrf_h);
            // If the sortition weight > 0 then this `id` is a block producer.
            if s_w > 0 {
                block_producers.insert(id.clone());
            }
            // If the sortition weight > 0 and this is our `id`, we have a slot to produce
            // the next block.
            if s_w > 0 && id.clone() == self_id {
                block_production_slot = Some(vrf_h.clone());
            }
            let v_w = util::percent_of(qty, msg.total_stake);
            validators.push((id.clone(), v_w));
        }

        // If we are the next block producer, generate a block if we can
        info!("[{}] is_block_producer = {:?}", "hail".blue(), block_production_slot.is_some());

        // Otherwise wait for the next block to be received
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct QueryIncomplete {
    pub block: Block,
    pub acks: Vec<Response>,
}

impl Handler<QueryIncomplete> for Hail {
    type Result = ();

    fn handle(&mut self, msg: QueryIncomplete, _ctx: &mut Context<Self>) -> Self::Result {
        ()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct QueryComplete {
    pub block: Block,
    pub acks: Vec<Response>,
}

impl Handler<QueryComplete> for Hail {
    type Result = ();

    fn handle(&mut self, msg: QueryComplete, _ctx: &mut Context<Self>) -> Self::Result {
        // FIXME: Verify that there are no duplicate ids
        let mut outcomes = vec![];
        for ack in msg.acks.iter() {
            match ack {
                Response::QueryBlockAck(qb_ack) => match self.committee.get(&qb_ack.id) {
                    Some((_, w)) => outcomes.push((qb_ack.id, w.clone(), qb_ack.outcome)),
                    None => (),
                },
                // FIXME: Error
                _ => (),
            }
        }
        // if yes: set_chit(tx, 1), update ancestral preferences
        if util::sum_outcomes(outcomes) > ALPHA {
            let vx = Vertex::new(msg.block.height, msg.block.hash());
            self.dag.set_chit(vx, 1).unwrap();
            // self.update_ancestral_preference(msg.block.hash()).unwrap();
            info!("[{}] query complete, chit = 1", "hail".blue());
        }
        // if no:  set_chit(tx, 0) -- happens in `insert_vx`
        // alpha::insert_block(&self.queried_blocks, msg.block.clone()).unwrap();
    }
}

// Instead of having an infinite loop as per the paper which receives and processes
// inbound unqueried blocks, we instead use the `Actor` and use `notify` whenever
// a fresh block is received.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Result<()>")]
pub struct FreshBlock {
    pub block: Block,
}

impl Handler<FreshBlock> for Hail {
    type Result = ResponseActFuture<Self, Result<()>>;

    fn handle(&mut self, msg: FreshBlock, _ctx: &mut Context<Self>) -> Self::Result {
        let validators = self.sample(ALPHA).unwrap();
        info!("[{}] sampled {:?}", "hail".blue(), validators.clone());
        let mut validator_ips = vec![];
        for (_, ip) in validators.iter() {
            validator_ips.push(ip.clone());
        }

        // Fanout queries to sampled validators
        let send_to_client = self.sender.send(Fanout {
            ips: validator_ips.clone(),
            request: Request::QueryBlock(QueryBlock { block: msg.block.clone() }),
        });

        // Wrap the future so that subsequent chained handlers can access te actor.
        let send_to_client = actix::fut::wrap_future::<_, Self>(send_to_client);

        let update_self = send_to_client.map(move |result, actor, ctx| {
            match result {
                Ok(acks) => {
                    // If the length of responses is the same as the length of the sampled ips,
                    // then every peer responded.
                    if acks.len() == validator_ips.len() {
                        Ok(ctx.notify(QueryComplete { block: msg.block.clone(), acks }))
                    } else {
                        Ok(ctx.notify(QueryIncomplete { block: msg.block.clone(), acks }))
                    }
                }
                Err(e) => Err(Error::Actix(e)),
            }
        });

        Box::pin(update_self)
    }
}

// Receiving blocks. The difference between receiving blocks and receiving a block query
// is that `ReceiveBlock` is used when we are the block producer generating a new block
// whereas a `QueryBlock` is used when receiving a block query from another validator.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "ReceiveBlockAck")]
pub struct ReceiveBlock {
    pub block: Block,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct ReceiveBlockAck;

impl Handler<ReceiveBlock> for Hail {
    type Result = ReceiveBlockAck;

    fn handle(&mut self, msg: ReceiveBlock, ctx: &mut Context<Self>) -> Self::Result {
        let block = msg.block.clone();
        //if !alpha::is_known_block(&self.known_blocks, block.hash()).unwrap() {
        info!("[{}] received new block {:?}", "hail".cyan(), block.clone());

        // let parents = self.select_parents(NPARENTS).unwrap();
        // self.insert(HailBlock::new(parents, block.clone())).unwrap();
        // alpha::insert_block(&self.known_blocks, block.clone()).unwrap();
        // ctx.notify(FreshBlock { block: block.clone() });
        // }
        ReceiveBlockAck {}
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "QueryBlockAck")]
pub struct QueryBlock {
    pub block: Block,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct QueryBlockAck {
    pub id: Id,
    pub block_hash: BlockHash,
    pub outcome: bool,
}

impl Handler<QueryBlock> for Hail {
    type Result = QueryBlockAck;

    fn handle(&mut self, msg: QueryBlock, ctx: &mut Context<Self>) -> Self::Result {
        let block = msg.block.clone();

        // FIXME: If we are in the middle of querying this transaction, wait until a
        // decision or a synchronous timebound is reached on attempts.
        // let outcome = self.is_strongly_preferred(msg.block.hash()).unwrap();
        QueryBlockAck { id: self.node_id, block_hash: msg.block.hash(), outcome: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct AcceptedCells {
    pub cells: Vec<Cell>,
}

impl Handler<AcceptedCells> for Hail {
    type Result = ();

    fn handle(&mut self, msg: AcceptedCells, _ctx: &mut Context<Self>) -> Self::Result {
        info!("[{}] received {} accepted cells", "hail".cyan(), msg.cells.len());
        // TODO ...
    }
}

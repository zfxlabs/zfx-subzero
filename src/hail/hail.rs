use crate::zfx_id::Id;
use zfx_sortition::sortition;

use crate::alpha::block::Block;
use crate::alpha::types::{BlockHash, BlockHeight, VrfOutput, Weight};
use crate::cell::Cell;
use crate::client::Fanout;
use crate::colored::Colorize;
use crate::graph::DAG;
use crate::protocol::{Request, Response};
use crate::storage::hail_block as block_storage;
use crate::util;

use super::block::HailBlock;
use super::conflict_map::ConflictMap;
use super::conflict_set::ConflictSet;
use super::vertex::Vertex;
use super::{Error, Result};

use tracing::{debug, error, info};

use actix::{Actor, AsyncContext, Context, Handler, Recipient, ResponseFuture};
use actix::{ActorFutureExt, ResponseActFuture, WrapFuture};

use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::net::SocketAddr;

// Safety parameters

const ALPHA: f64 = 0.5;
const BETA1: u8 = 11;
const BETA2: u8 = 20;

/// Hail is a Snow* based consensus for blocks.
pub struct Hail {
    /// The hash of the last accepted block (at the current block height).
    last_accepted_hash: Option<BlockHash>,
    /// The current block height.
    height: BlockHeight,
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
    /// A mapping of block hashes to live blocks.
    live_blocks: HashMap<BlockHash, Block>,
    /// The map contains vertices (height, block hash) which are already accepted
    accepted_vertices: HashSet<Vertex>,
    /// The consensus graph.
    dag: DAG<Vertex>,
    /// The block production vrf (a vrf for `height + 1` if we are the next block producer).
    block_production_slot: Option<VrfOutput>,
    /// Whether we have already proposed a block at this height.
    block_proposed: bool,
}

impl Hail {
    /// Hail is initialised with the most recent `frontier`, which is the last set of
    /// blocks yet to become final.
    pub fn new(sender: Recipient<Fanout>, node_id: Id) -> Self {
        Hail {
            last_accepted_hash: None,
            height: 0,
            sender,
            node_id,
            committee: HashMap::default(),
            known_blocks: sled::Config::new().temporary(true).open().unwrap(),
            queried_blocks: sled::Config::new().temporary(true).open().unwrap(),
            conflict_map: ConflictMap::new(),
            live_blocks: HashMap::default(),
            accepted_vertices: HashSet::new(),
            dag: DAG::new(),
            block_production_slot: None,
            block_proposed: false,
        }
    }

    /// Called for blocks which are received via consensus queries.
    /// Returns `true` if the block hasn't been encountered before.
    fn on_receive_block(&mut self, hail_block: HailBlock) -> Result<bool> {
        if !block_storage::is_known_block(&self.known_blocks, hail_block.hash()?).unwrap() {
            self.insert(hail_block.clone())?;
            let _ = block_storage::insert_block(&self.known_blocks, hail_block.clone());
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // Vertices

    pub fn insert(&mut self, block: HailBlock) -> Result<()> {
        let inner_block = block.inner();
        let vertex = block.vertex().unwrap();
        self.conflict_map.insert_block(inner_block.clone())?;
        self.dag.insert_vx(vertex, vec![block.parent()])?;
        Ok(())
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
    /// search until a preferrential parent with height = `h - 1` is found.
    pub fn select_parent(&mut self, h: BlockHeight) -> Result<Vertex> {
        if self.dag.is_empty() {
            return Err(Error::EmptyDAG);
        }
        let leaves = self.dag.leaves();
        for leaf in leaves {
            for vx in self.dag.dfs(&leaf) {
                if self.is_strongly_preferred(vx.clone())? && vx.height == h {
                    return Ok(vx.clone());
                }
            }
        }
        Err(Error::InvalidParent)
    }

    // Ancestral Preference

    // The ancestral update updates the preferred path through the DAG every time a new
    // vertex is added.
    pub fn update_ancestral_preference(&mut self, root_vx: Vertex) -> Result<()> {
        for vx in self.dag.dfs(&root_vx) {
            // conviction of T vs Pt.pref
            let pref = self.conflict_map.get_preferred(&vx.height)?;
            let d1 = self.dag.conviction(vx.clone())?;
            let d2 = self.dag.conviction(Vertex::new(vx.height, pref))?;
            // update the conflict set at this tx
            self.conflict_map.update_conflict_set(
                vx.height.clone(),
                vx.block_hash.clone(),
                d1,
                d2,
            )?;
        }
        Ok(())
    }

    // Finality

    /// Checks whether the block at `BlockHeight` is accepted as final.
    pub fn is_accepted_block(&self, vx: &Vertex) -> Result<bool> {
        if self.conflict_map.is_singleton(&vx.height)?
            && self.conflict_map.get_confidence(&vx)? >= BETA1
        {
            Ok(true)
        } else {
            if self.conflict_map.get_confidence(&vx)? >= BETA2 {
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }

    /// Checks whether the parent of the provided `TxHash` is final - note that we do not
    /// traverse all of the parents of the accepted parent, since a child transaction
    /// cannot be final if its parent is not also final.
    pub fn is_accepted(&self, initial_vertex: &Vertex) -> Result<bool> {
        let mut parent_accepted = true;
        match self.dag.get(initial_vertex) {
            Some(parents) => {
                for parent in parents.iter() {
                    if !self.is_accepted_block(&parent)? {
                        parent_accepted = false;
                        break;
                    }
                }
            }
            None => return Err(Error::InvalidBlockHash(initial_vertex.block_hash.clone())),
        }
        if parent_accepted {
            self.is_accepted_block(initial_vertex)
        } else {
            Ok(false)
        }
    }

    // Accepted Frontier

    /// The accepted frontier of the DAG is a depth-first-search on the leaves of the DAG
    /// up to a vertices considered final, collecting all the final nodes.
    pub fn get_accepted_frontier(&self) -> Result<Vec<Vertex>> {
        if self.dag.is_empty() {
            return Ok(vec![]);
        }
        let mut accepted_frontier = vec![];
        let leaves = self.dag.leaves();
        for leaf in leaves {
            for vx in self.dag.dfs(&leaf) {
                if self.is_accepted(vx)? {
                    accepted_frontier.push(vx.clone());
                    break;
                }
            }
        }
        Ok(accepted_frontier)
    }

    /// Check if a transaction or one of its ancestors have become accepted
    pub fn compute_accepted_vertices(&mut self, vx: &Vertex) -> Result<Vec<Vertex>> {
        let mut new = vec![];
        for t in self.dag.dfs(vx) {
            if !self.accepted_vertices.contains(t) && self.is_accepted(t)? {
                new.push(t.clone());
                let _ = self.accepted_vertices.insert(t.clone());
            }
        }
        Ok(new)
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
    pub last_accepted_hash: BlockHash,
    pub height: u64,
    pub self_id: Id,
    pub self_staking_capacity: u64,
    pub total_staking_capacity: u64,
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

        let mut committee = HashMap::default();
        let mut block_producers = HashSet::new();
        let mut block_production_slot = None;
        info!("[{}] total_staking_capacity = {:?}", "hail".blue(), msg.total_staking_capacity);
        for (id, (ip, qty)) in msg.validators {
            let vrf_h = compute_vrf_h(id.clone(), &msg.vrf_out);
            let s_w = sortition::select(qty, msg.total_staking_capacity, expected_size, &vrf_h);
            info!("id = {:?}, qty = {:?}", id.clone(), qty.clone());
            // If the sortition weight > 0 then this `id` is a block producer.
            if s_w > 0 {
                block_producers.insert(id.clone());
            }
            let v_w = util::percent_of(qty, msg.total_staking_capacity);
            if let Some(_) = committee.insert(id.clone(), (ip, v_w)) {
                panic!("duplicate validator insertion");
            }
        }

        // Compute whether we are a block producer
        let vrf_h = compute_vrf_h(self_id.clone(), &msg.vrf_out);
        let s_w = sortition::select(
            msg.self_staking_capacity,
            msg.total_staking_capacity,
            expected_size,
            &vrf_h,
        );
        if s_w > 0 {
            block_production_slot = Some(vrf_h.clone());
        }

        info!("[{}] is_block_producer = {:?}", "hail".blue(), block_production_slot.is_some());
        info!(
            "[{}] last_accepted_hash = {}",
            "hail".blue(),
            hex::encode(msg.last_accepted_hash.clone())
        );

        self.last_accepted_hash = Some(msg.last_accepted_hash);
        self.height = msg.height;
        self.block_production_slot = block_production_slot.clone();
        self.block_proposed = false;
        self.committee = committee;

        // TODO: Check if we have pending accepted cells and build a block (block building
        // will still take place when receiving accepted cells otherwise).
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct QueryIncomplete {
    pub block: HailBlock,
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
    pub block: HailBlock,
    pub acks: Vec<Response>,
}

impl Handler<QueryComplete> for Hail {
    type Result = ();

    fn handle(&mut self, msg: QueryComplete, ctx: &mut Context<Self>) -> Self::Result {
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
            let vx = msg.block.vertex().unwrap();
            self.dag.set_chit(vx.clone(), 1).unwrap();
            self.update_ancestral_preference(vx.clone()).unwrap();
            info!("[{}] query complete, chit = 1", "hail".blue());
            // Let `hail` know that this block can now be built upon
            let _ = self.live_blocks.insert(vx.block_hash.clone(), msg.block.inner());

            // The block or some of its ancestors may have become accepted. Check this.
            let newly_accepted = self.compute_accepted_vertices(&vx);
            match newly_accepted {
                Ok(newly_accepted) => {
                    if !newly_accepted.is_empty() {
                        ctx.notify(NewAccepted { vertices: newly_accepted });
                    }
                }
                Err(e) => {
                    // Its a bug if this occurs
                    panic!("[hail] Error checking whether block is accepted: {}", e);
                }
            }
        }
        // if no:  set_chit(tx, 0) -- happens in `insert_vx`
        block_storage::insert_block(&self.queried_blocks, msg.block.clone()).unwrap();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct NewAccepted {
    pub vertices: Vec<Vertex>,
}
impl Handler<NewAccepted> for Hail {
    type Result = ();

    fn handle(&mut self, msg: NewAccepted, _ctx: &mut Context<Self>) -> Self::Result {
        let mut blocks = vec![];
        for vx in msg.vertices.iter().cloned() {
            // At this point we can be sure that the tx is known
            let (_, block) = block_storage::get_block(&self.known_blocks, vx.block_hash).unwrap();
            info!("[{}] transaction is accepted\n{}", "hail".blue(), block.clone());
            blocks.push(block.inner());
        }
        // TODO: There should only be one accepted block
        // let _ = self.alpha_recipient.do_send(AcceptedBlocks { blocks });
    }
}

// Instead of having an infinite loop as per the paper which receives and processes
// inbound unqueried blocks, we instead use the `Actor` and use `notify` whenever
// a fresh block is received.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Result<()>")]
pub struct FreshBlock {
    pub block: HailBlock,
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
                // FIXME
                Err(_) => Err(Error::ActixMailboxError),
            }
        });

        Box::pin(update_self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "QueryBlockAck")]
pub struct QueryBlock {
    pub block: HailBlock,
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
        let vx = msg.block.vertex().unwrap();
        info!(
            "[{}] received query for block {}",
            "hail".blue(),
            hex::encode(vx.block_hash.clone())
        );
        match self.on_receive_block(msg.block.clone()) {
            Ok(true) => ctx.notify(FreshBlock { block: msg.block.clone() }),
            Ok(false) => (),
            Err(e) => {
                error!("[{}] failed to receive block {:?}: {}", "hail".blue(), msg.block, e);
            }
        }
        // FIXME: If we are in the middle of querying this block, wait until a decision or a
        // synchronous timebound is reached on attempts.
        match self.is_strongly_preferred(vx.clone()) {
            Ok(outcome) => {
                QueryBlockAck { id: self.node_id, block_hash: vx.block_hash.clone(), outcome }
            }
            Err(e) => {
                error!("[{}] Missing ancestor or {}\n {}", "hail".blue(), msg.block, e);
                // FIXME: We're voting against the block w/o enough information
                QueryBlockAck {
                    id: self.node_id,
                    block_hash: vx.block_hash.clone(),
                    outcome: false,
                }
            }
        }
    }
}

// Allow clients to fetch blocks for testing.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "BlockAck")]
pub struct GetBlock {
    pub block_hash: BlockHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct BlockAck {
    pub block: Option<Block>,
}

impl Handler<GetBlock> for Hail {
    type Result = BlockAck;

    fn handle(&mut self, msg: GetBlock, _ctx: &mut Context<Self>) -> Self::Result {
        BlockAck { block: self.live_blocks.get(&msg.block_hash).map(|x| x.clone()) }
    }
}

// Generate blocks

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "GenerateBlockAck")]
pub struct GenerateBlock {
    pub block: Block,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct GenerateBlockAck {
    /// hash of applied transaction
    pub block_hash: Option<BlockHash>,
}

impl Handler<GenerateBlock> for Hail {
    type Result = GenerateBlockAck;

    fn handle(&mut self, msg: GenerateBlock, ctx: &mut Context<Self>) -> Self::Result {
        let parent = self.select_parent(msg.block.height).unwrap();
        let hail_block = HailBlock::new(parent, msg.block.clone());
        info!("[{}] generating new block\n{}", "hail".blue(), hail_block.clone());

        match self.on_receive_block(hail_block.clone()) {
            Ok(true) => {
                ctx.notify(FreshBlock { block: hail_block });
                GenerateBlockAck { block_hash: Some(msg.block.hash().unwrap()) }
            }
            Ok(false) => GenerateBlockAck { block_hash: None },

            Err(e) => {
                error!("[{}] couldn't insert new block\n{}:\n {}", "hail".blue(), hail_block, e);
                GenerateBlockAck { block_hash: None }
            }
        }
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

        match self.block_production_slot {
            Some(vrf_out) => {
                // If we are the block producer at height `h + 1` then generate a new block with the
                // accepted cells.
                let block = Block::new(
                    self.last_accepted_hash.unwrap(),
                    self.height + 1,
                    vrf_out,
                    msg.cells.clone(),
                );
                // ctx.notify(GenerateBlock { block });
            }
            None =>
            // If we are not a block producer then do nothing with the accepted cells.
            {
                ()
            }
        }
    }
}

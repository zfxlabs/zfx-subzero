use crate::colored::Colorize;
use crate::zfx_id::Id;

use crate::alpha::types::{TxHash, Weight};
use crate::cell::types::CellHash;
use crate::cell::{Cell, CellIds};
use crate::client::Fanout;
use crate::graph::conflict_graph::ConflictGraph;
use crate::graph::DAG;
use crate::hail::AcceptedCells;
use crate::protocol::{Request, Response};
use crate::storage::tx as tx_storage;
use crate::util;

use super::tx::Tx;
use super::{Error, Result};

use tracing::{debug, error, info};

use actix::{Actor, AsyncContext, Context, Handler, Recipient};
use actix::{ActorFutureExt, ResponseActFuture, ResponseFuture};

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

// Parent selection

const NPARENTS: usize = 3;

// Safety parameters

pub const ALPHA: f64 = 0.5;
pub const BETA1: u8 = 11;
pub const BETA2: u8 = 20;

/// Sleet is a consensus bearing `mempool` for transactions conflicting on spent inputs.
pub struct Sleet {
    /// The client used to make external requests.
    sender: Recipient<Fanout>,
    /// Connection to Hail
    hail_recipient: Recipient<AcceptedCells>,
    /// The identity of this validator.
    node_id: Id,
    /// The weighted validator set.
    committee: HashMap<Id, (SocketAddr, Weight)>,
    /// The set of all known transactions in storage.
    known_txs: sled::Db,
    /// The set of all queried transactions in storage.
    queried_txs: sled::Db,
    /// The graph of conflicting transactions (potentially multi-input).
    conflict_graph: ConflictGraph,
    /// A mapping of a cell hashes to unspent cells.
    live_cells: HashMap<CellHash, Cell>,
    /// The map contains transactions already accepted
    accepted_txs: HashSet<TxHash>,
    /// The map contains transactions rejected because they conflict with an accepted transaction
    /// Note: we rely heavily on the fact that transacrions have the same hash as the wrapped cell
    rejected_txs: HashSet<TxHash>,
    /// The consensus graph.
    dag: DAG<TxHash>,
}

impl Sleet {
    // Initialisation - FIXME: Temporary databases
    pub fn new(
        sender: Recipient<Fanout>,
        hail_recipient: Recipient<AcceptedCells>,
        node_id: Id,
    ) -> Self {
        Sleet {
            sender,
            hail_recipient,
            node_id,
            committee: HashMap::default(),
            known_txs: sled::Config::new().temporary(true).open().unwrap(),
            queried_txs: sled::Config::new().temporary(true).open().unwrap(),
            conflict_graph: ConflictGraph::new(CellIds::empty()),
            live_cells: HashMap::default(),
            accepted_txs: HashSet::new(),
            rejected_txs: HashSet::new(),
            dag: DAG::new(),
        }
    }

    /// Called for all newly discovered transactions.
    /// Returns `true` if the transaction haven't been encountered before
    fn on_receive_tx(&mut self, sleet_tx: Tx) -> Result<bool> {
        // Skip adding coinbase transactions (block rewards / initial allocations) to the
        // mempool.
        if has_coinbase_output(&sleet_tx.cell) {
            return Err(Error::InvalidCoinbaseTransaction(sleet_tx.cell));
        }
        if !tx_storage::is_known_tx(&self.known_txs, sleet_tx.hash()).unwrap() {
            self.insert(sleet_tx.clone())?;
            let _ = tx_storage::insert_tx(&self.known_txs, sleet_tx.clone());
            Ok(true)
        } else {
            // info!("[{}] received already known transaction {}", "sleet".cyan(), tx.clone());
            Ok(false)
        }
    }

    // Vertices

    pub fn insert(&mut self, tx: Tx) -> Result<()> {
        let cell = tx.cell.clone();
        self.conflict_graph.insert_cell(cell.clone())?;
        self.dag.insert_vx(tx.hash(), tx.parents.clone())?;
        Ok(())
    }

    // Branch preference

    /// Starts at some vertex and does a depth first search in order to compute whether
    /// the vertex is strongly preferred (by checking whether all its ancestry is
    /// preferred).
    pub fn is_strongly_preferred(&self, tx: TxHash) -> Result<bool> {
        for ancestor in self.dag.dfs(&tx) {
            if !self.conflict_graph.is_preferred(ancestor)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    // Adaptive Parent Selection

    /// Starts at the live edges (the leaf nodes) of the `DAG` and does a depth first
    /// search until `p` preferrential parents are accumulated (or none if there are
    /// none).
    pub fn select_parents(&self, p: usize) -> Result<Vec<TxHash>> {
        if self.dag.is_empty() {
            return Ok(vec![]);
        }
        let mut parents = vec![];
        // vertices to exclude from selection, because they are accessible from a parent
        let mut accessible = vec![];
        let leaves = self.dag.leaves();
        'outer: for leaf in leaves {
            for elt in self.dag.dfs(&leaf) {
                if parents.len() >= p {
                    // Found `p` preferred parents.
                    break 'outer;
                }
                if self.is_strongly_preferred(elt.clone())?
                    && !parents.contains(elt)
                    && !accessible.contains(elt)
                {
                    parents.push(elt.clone());
                    accessible.extend(self.dag.dfs(elt));
                    // Found a preferred parent for this leaf so skip.
                    break;
                }
            }
        }
        Ok(parents)
    }

    // Ancestral Preference

    // The ancestral update updates the preferred path through the DAG every time a new
    // vertex is added.
    pub fn update_ancestral_preference(&mut self, root_txhash: TxHash) -> Result<()> {
        for tx_hash in self.dag.dfs(&root_txhash) {
            // conviction of T vs Pt.pref
            let pref = self.conflict_graph.get_preferred(&tx_hash)?;
            let d1 = self.dag.conviction(tx_hash.clone())?;
            let d2 = self.dag.conviction(pref)?;
            // update the conflict set at this tx
            self.conflict_graph.update_conflict_set(&tx_hash, d1, d2)?;
        }
        Ok(())
    }

    // Finality

    /// Checks whether the transaction `TxHash` is accepted as final.
    pub fn is_accepted_tx(&self, tx_hash: &TxHash) -> bool {
        // It's a bug if we check a non-existent transaction
        let confidence = match self.conflict_graph.get_confidence(tx_hash) {
            Ok(c) => c,
            Err(e) => panic!("{}", e),
        };
        if self.conflict_graph.is_singleton(tx_hash).unwrap() && confidence >= BETA1 {
            true
        } else if confidence >= BETA2 {
            true
        } else {
            false
        }
    }

    /// Checks whether the ancestry of the provided `TxHash` is final
    pub fn is_accepted(&self, initial_tx_hash: &TxHash) -> bool {
        for tx in self.dag.dfs(initial_tx_hash) {
            if !self.is_accepted_tx(tx) {
                return false;
            }
        }
        return true;
    }

    /// Memoising version of `is_accepted`.
    /// Rationale: `is_accepted` itself contains a DFS loop; also, its callsites are DFS loops
    /// walking the graph "upwards", so most values have already been calculated in previous iterations
    pub fn is_accepted_memo(&self, tx_hash: &TxHash, memo: &mut HashMap<TxHash, bool>) -> bool {
        if let Some(res) = memo.get(tx_hash) {
            *res
        } else {
            let res = self.is_accepted(tx_hash);
            let _ = memo.insert(tx_hash.clone(), res);
            res
        }
    }

    /// Clean up the conflict graph and the DAG
    /// Returns the children of rejected transactions
    pub fn remove_conflicts(&mut self, tx_hash: &TxHash, tx: &Tx) -> Result<HashSet<CellHash>> {
        let rejected = self.conflict_graph.accept_cell(tx.cell.clone())?;
        let mut children = HashSet::new();
        for hash in rejected {
            let _ = self.rejected_txs.insert(hash.clone());
            let ch = self.dag.remove_vx(&hash)?;
            children.extend(ch.iter());
        }

        Ok(children)
    }

    // Accepted Frontier

    /// The accepted frontier of the DAG is a depth-first-search on the leaves of the DAG
    /// up to a vertices considered final, collecting all the final nodes.
    pub fn get_accepted_frontier(&self) -> HashSet<TxHash> {
        let mut accepted_frontier = HashSet::new();
        if self.dag.is_empty() {
            return accepted_frontier;
        }
        let mut above_frontier: HashSet<TxHash> = HashSet::new();
        let leaves = self.dag.leaves();
        let mut memo = HashMap::new();
        for leaf in leaves {
            for tx_hash in self.dag.dfs(&leaf) {
                if !above_frontier.contains(tx_hash) && self.is_accepted_memo(tx_hash, &mut memo) {
                    let _ = accepted_frontier.insert(tx_hash.clone());
                    above_frontier.extend(self.dag.dfs(tx_hash));
                }
            }
        }
        accepted_frontier
    }

    /// Remove transactions from the dag above the accepted frontier
    pub fn prune_at_accepted_frontier(&mut self) {
        let frontier = self.get_accepted_frontier();
        let mut to_be_pruned = HashSet::new();
        for f in frontier.iter() {
            to_be_pruned.extend(self.dag.dfs(f));
        }
        for a in to_be_pruned.iter() {
            if !frontier.contains(a) {
                info!("Pruned {}", hex::encode(a));
                let _ = self.dag.remove_vx(a);
            }
        }
    }

    /// Check if a transaction or one of its ancestors have become accepted
    pub fn compute_accepted_txs(&mut self, tx_hash: &TxHash) -> Vec<TxHash> {
        let mut new = vec![];
        let mut memo = HashMap::new();
        for t in self.dag.dfs(tx_hash) {
            if !self.accepted_txs.contains(t) && self.is_accepted_memo(t, &mut memo) {
                new.push(t.clone());
                let _ = self.accepted_txs.insert(t.clone());
            }
        }
        new
    }

    // Weighted sampling

    pub fn sample(&self, minimum_weight: Weight) -> Result<Vec<(Id, SocketAddr)>> {
        let mut validators = vec![];
        for (id, (ip, w)) in self.committee.iter() {
            validators.push((id.clone(), ip.clone(), w.clone()));
        }
        util::sample_weighted(minimum_weight, validators).ok_or(Error::InsufficientWeight)
    }
}

// TODO: this function should probably moved elsewhere
/// Check if a cell creates a coinbase output.
pub fn has_coinbase_output(cell: &Cell) -> bool {
    for o in cell.outputs().iter() {
        if o.cell_type == crate::cell::CellType::Coinbase {
            return true;
        }
    }
    false
}

impl Actor for Sleet {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
        debug!("started sleet");
    }
}

// When the committee is initialised in `alpha` or when it comes back online due to a
// `FaultyNetwork` message received in `alpha`, `sleet` is updated with the latest relevant
// chain state.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct LiveCommittee {
    pub validators: HashMap<Id, (SocketAddr, f64)>,
    pub live_cells: HashMap<CellHash, Cell>,
}

impl Handler<LiveCommittee> for Sleet {
    type Result = ();

    fn handle(&mut self, msg: LiveCommittee, _ctx: &mut Context<Self>) -> Self::Result {
        // Build the list of available UTXOs
        let txs_len = format!("{:?}", msg.live_cells.len());
        info!(
            "\n{} received {} transactions containing spendable outputs",
            "[sleet]".cyan(),
            txs_len.cyan()
        );
        let mut cell_ids_set: CellIds = CellIds::empty();
        for (cell_hash, cell) in msg.live_cells.clone() {
            info!("{}", cell.clone());
            let cell_ids = CellIds::from_outputs(cell_hash.clone(), cell.outputs()).unwrap();
            cell_ids_set = cell_ids_set.union(&cell_ids).cloned().collect();
        }
        info!("");
        self.live_cells = msg.live_cells.clone();
        self.conflict_graph = ConflictGraph::new(cell_ids_set);

        let mut s: String = format!("<<{}>>\n", "sleet".cyan());
        for (id, (_, w)) in msg.validators.clone() {
            let id_s = format!("{:?}", id).yellow();
            let w_s = format!("{:?}", w).cyan();
            s = format!("{} ν = {} {} | {} {}\n", s, "⦑".magenta(), id_s, w_s, "⦒".magenta());
        }
        info!("{}", s);

        self.committee = msg.validators;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct QueryIncomplete {
    pub tx: Tx,
    pub acks: Vec<Response>,
}

impl Handler<QueryIncomplete> for Sleet {
    type Result = ();

    fn handle(&mut self, _msg: QueryIncomplete, _ctx: &mut Context<Self>) -> Self::Result {
        ()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct QueryComplete {
    pub tx: Tx,
    pub acks: Vec<Response>,
}

impl Handler<QueryComplete> for Sleet {
    type Result = ();

    fn handle(&mut self, msg: QueryComplete, ctx: &mut Context<Self>) -> Self::Result {
        // FIXME: Verify that there are no duplicate ids
        let mut outcomes = vec![];
        for ack in msg.acks.iter() {
            match ack {
                Response::QueryTxAck(qtx_ack) => match self.committee.get(&qtx_ack.id) {
                    Some((_, w)) => outcomes.push((qtx_ack.id, w.clone(), qtx_ack.outcome)),
                    None => (),
                },
                // FIXME: Error
                _ => (),
            }
        }
        //   if yes: set_chit(tx, 1), update ancestral preferences
        if util::sum_outcomes(outcomes) > ALPHA {
            self.dag.set_chit(msg.tx.hash(), 1).unwrap();
            self.update_ancestral_preference(msg.tx.hash()).unwrap();
            info!("[{}] query complete, chit = 1", "sleet".cyan());
            // Let `sleet` know that you can now build on this tx
            let _ = self.live_cells.insert(msg.tx.cell.hash(), msg.tx.cell.clone());

            // The transaction or some of its ancestors may have become
            // accepted. Check this.
            let new_accepted = self.compute_accepted_txs(&msg.tx.hash());
            if !new_accepted.is_empty() {
                ctx.notify(NewAccepted { tx_hashes: new_accepted });
            }
        }
        //   if no:  set_chit(tx, 0) -- happens in `insert_vx`
        tx_storage::insert_tx(&self.queried_txs, msg.tx.clone()).unwrap();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct NewAccepted {
    pub tx_hashes: Vec<TxHash>,
}
impl Handler<NewAccepted> for Sleet {
    type Result = ();

    fn handle(&mut self, msg: NewAccepted, _ctx: &mut Context<Self>) -> Self::Result {
        let mut cells = vec![];

        self.prune_at_accepted_frontier();

        for tx_hash in msg.tx_hashes.iter().cloned() {
            // At this point we can be sure that the tx is known
            let (_, tx) = tx_storage::get_tx(&self.known_txs, tx_hash).unwrap();

            // TODO we most likely will need to re-issue the children of rejected transactions
            //      with better parents
            let _children_of_rejected = self.remove_conflicts(&tx_hash, &tx);
            info!("[{}] transaction is accepted\n{}", "sleet".cyan(), tx.clone());
            cells.push(tx.cell);
        }
        let _ = self.hail_recipient.do_send(AcceptedCells { cells });
    }
}

// Instead of having an infinite loop as per the paper which receives and processes
// inbound unqueried transactions, we instead use the `Actor` and use `notify` whenever
// a fresh transaction is received - either externally in `GenerateTx` or as an internal
// consensus message via `QueryTx`.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Result<()>")]
pub struct FreshTx {
    pub tx: Tx,
}

impl Handler<FreshTx> for Sleet {
    type Result = ResponseActFuture<Self, Result<()>>;

    fn handle(&mut self, msg: FreshTx, _ctx: &mut Context<Self>) -> Self::Result {
        let validators = self.sample(ALPHA).unwrap();
        info!("[{}] Querying\n{}", "sleet".cyan(), msg.tx.clone());
        info!("[{}] sampled {:?}", "sleet".cyan(), validators.clone());
        let mut validator_ips = vec![];
        for (_, ip) in validators.iter() {
            validator_ips.push(ip.clone());
        }

        // Fanout queries to sampled validators
        let send_to_client = self.sender.send(Fanout {
            ips: validator_ips.clone(),
            request: Request::QueryTx(QueryTx { tx: msg.tx.clone() }),
        });

        // Wrap the future so that subsequent chained handlers can access te actor.
        let send_to_client = actix::fut::wrap_future::<_, Self>(send_to_client);

        let update_self = send_to_client.map(move |result, _actor, ctx| {
            match result {
                Ok(acks) => {
                    // If the length of responses is the same as the length of the sampled ips,
                    // then every peer responded.
                    if acks.len() == validator_ips.len() {
                        Ok(ctx.notify(QueryComplete { tx: msg.tx.clone(), acks }))
                    } else {
                        Ok(ctx.notify(QueryIncomplete { tx: msg.tx.clone(), acks }))
                    }
                }
                Err(e) => Err(Error::Actix(e)),
            }
        });

        Box::pin(update_self)
    }
}

// Allow clients to fetch transactions for testing.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "CellAck")]
pub struct GetCell {
    pub cell_hash: CellHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct CellAck {
    pub cell: Option<Cell>,
}

impl Handler<GetCell> for Sleet {
    type Result = CellAck;

    fn handle(&mut self, msg: GetCell, _ctx: &mut Context<Self>) -> Self::Result {
        CellAck { cell: self.live_cells.get(&msg.cell_hash).map(|x| x.clone()) }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "GenerateTxAck")]
pub struct GenerateTx {
    pub cell: Cell,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct GenerateTxAck {
    /// hash of applied transaction
    pub cell_hash: Option<CellHash>,
}

impl Handler<GenerateTx> for Sleet {
    type Result = GenerateTxAck;

    fn handle(&mut self, msg: GenerateTx, ctx: &mut Context<Self>) -> Self::Result {
        let parents = self.select_parents(NPARENTS).unwrap();
        let sleet_tx = Tx::new(parents, msg.cell.clone());
        info!("[{}] Generating new transaction\n{}", "sleet".cyan(), sleet_tx.clone());

        match self.on_receive_tx(sleet_tx.clone()) {
            Ok(true) => {
                ctx.notify(FreshTx { tx: sleet_tx });
                GenerateTxAck { cell_hash: Some(msg.cell.hash()) }
            }
            Ok(false) => GenerateTxAck { cell_hash: None },

            Err(e) => {
                error!(
                    "[{}] Couldn't insert new transaction\n{}:\n {}",
                    "sleet".cyan(),
                    sleet_tx,
                    e
                );
                GenerateTxAck { cell_hash: None }
            }
        }
    }
}

// Receiving transactions. The only difference between receiving transactions and receiving
// a transaction query is that any client should be able to send `sleet` a `GenerateTx`
// message, whereas only network validators should be able to perform a `QueryTx`.
//
// Otherwise the functionality is identical but `QueryTx` returns a consensus response -
// whether the transaction is strongly preferred or not.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "QueryTxAck")]
pub struct QueryTx {
    pub tx: Tx,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct QueryTxAck {
    pub id: Id,
    pub tx_hash: TxHash,
    pub outcome: bool,
}

impl Handler<QueryTx> for Sleet {
    type Result = ResponseFuture<QueryTxAck>;

    fn handle(&mut self, msg: QueryTx, ctx: &mut Context<Self>) -> Self::Result {
        info!("[{}] Received query for transaction {}", "sleet".cyan(), hex::encode(msg.tx.hash()));
        let id = self.node_id.clone();
        let tx_hash = msg.tx.hash();
        match self.on_receive_tx(msg.tx.clone()) {
            Ok(is_new) => {
                if is_new {
                    ctx.notify(FreshTx { tx: msg.tx.clone() });
                };

                // We may have accepted or rejected the transaction already when the query comes in
                if self.accepted_txs.contains(&tx_hash) {
                    return Box::pin(async move { QueryTxAck { id, tx_hash, outcome: true } });
                }
                if self.rejected_txs.contains(&tx_hash) {
                    return Box::pin(async move { QueryTxAck { id, tx_hash, outcome: false } });
                }

                // FIXME: If we are in the middle of querying this transaction, wait until a
                // decision or a synchronous timebound is reached on attempts.
                let outcome = self.is_strongly_preferred(tx_hash.clone()).unwrap();
                Box::pin(async move { QueryTxAck { id, tx_hash, outcome } })
            }
            Err(e) => {
                info!(
                    "[{}] Couldn't insert queried transaction {:?}: {}",
                    "sleet".cyan(),
                    msg.tx,
                    e
                );
                return Box::pin(async move { QueryTxAck { id, tx_hash, outcome: false } });
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "CellHashes")]
pub struct GetCellHashes;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct CellHashes {
    pub ids: Vec<CellHash>,
}

impl Handler<GetCellHashes> for Sleet {
    type Result = CellHashes;

    fn handle(&mut self, _msg: GetCellHashes, _ctx: &mut Context<Self>) -> Self::Result {
        return CellHashes { ids: self.live_cells.keys().cloned().collect::<Vec<CellHash>>() };
    }
}

#[cfg(test)]
mod sleet_tests;

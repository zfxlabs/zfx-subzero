use crate::colored::Colorize;
use crate::zfx_id::Id;

use crate::alpha::types::{TxHash, Weight};
use crate::cell::types::CellHash;
use crate::cell::{Cell, CellIds};
use crate::client::{ClientRequest, ClientResponse};
use crate::graph::conflict_graph::ConflictGraph;
use crate::graph::DAG;
use crate::hail::AcceptedCells;
use crate::protocol::{Request, Response};
use crate::storage::tx as tx_storage;
use crate::util;

use super::tx::{Tx, TxStatus};
use super::{Error, Result};

use tracing::{debug, error, info};

use actix::WrapFuture;
use actix::{Actor, AsyncContext, Context, Handler, Recipient};
use actix::{ActorFutureExt, ResponseActFuture, ResponseFuture};

use tokio::sync::oneshot;
use tokio::time::{self, Duration};

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;

use self::sleet_utils::{BoundedHashMap, BoundedHashSet};
mod sleet_utils;

// Parent selection

const NPARENTS: usize = 3;

// Safety parameters

pub const ALPHA: f64 = 0.5;
pub const BETA1: u8 = 11;
pub const BETA2: u8 = 20;

// Constants

/// Timeout for answering a `QueryTx` message
const QUERY_RESPONSE_TIMEOUT_MS: u64 = 5000;

/// Sleet is a consensus bearing `mempool` for transactions conflicting on spent inputs.
pub struct Sleet {
    /// The client used to make external requests.
    sender: Recipient<ClientRequest>,
    /// Connection to Hail
    hail_recipient: Recipient<AcceptedCells>,
    /// The identity of this validator.
    node_id: Id,
    node_ip: SocketAddr,
    /// The weighted validator set.
    committee: HashMap<Id, (SocketAddr, Weight)>,
    /// The set of all known transactions in storage.
    known_txs: sled::Db,
    /// The graph of conflicting transactions (potentially multi-input).
    conflict_graph: ConflictGraph,
    /// A mapping of a cell hashes to unspent cells.
    live_cells: BoundedHashMap<CellHash, Cell>,
    /// The map contains transactions already accepted, used by the integration tests
    accepted_txs: BoundedHashSet<TxHash>,
    /// Incoming queries pending that couldn't be processed because of missing ancestry
    pending_queries: Vec<(Tx, oneshot::Sender<bool>)>,
    /// The consensus graph. Contains the accepted frontier and the undecided transactions
    dag: DAG<TxHash>,
    /// The accepted frontier of the DAG is a depth-first-search on the leaves of the DAG
    /// up to a vertices considered final, collecting all the final nodes.
    accepted_frontier: HashSet<TxHash>,
}

impl Sleet {
    // Initialisation - FIXME: Temporary databases
    pub fn new(
        sender: Recipient<ClientRequest>,
        hail_recipient: Recipient<AcceptedCells>,
        node_id: Id,
        node_ip: SocketAddr,
    ) -> Self {
        Sleet {
            sender,
            hail_recipient,
            node_id,
            node_ip,
            committee: HashMap::default(),
            known_txs: sled::Config::new().temporary(true).open().unwrap(),
            conflict_graph: ConflictGraph::new(CellIds::empty()),
            live_cells: BoundedHashMap::new(1000),
            accepted_txs: BoundedHashSet::new(1000),
            pending_queries: vec![],
            dag: DAG::new(),
            accepted_frontier: HashSet::new(),
        }
    }

    /// Called for all newly discovered transactions.
    /// Returns `true` if the transaction haven't been encountered before
    fn on_receive_tx(&mut self, mut sleet_tx: Tx) -> Result<bool> {
        // Skip adding coinbase transactions (block rewards / initial allocations) to the
        // mempool.
        if util::has_coinbase_output(&sleet_tx.cell) {
            return Err(Error::InvalidCoinbaseTransaction(sleet_tx.cell));
        }

        // Insert transaction if it is new, or it is a re-issued transaction that
        // was removed due to conflicting ancestry
        if !tx_storage::is_known_tx(&self.known_txs, sleet_tx.hash()).unwrap()
            || tx_storage::is_removed_tx(&self.known_txs, &sleet_tx.hash()).unwrap()
        {
            if !self.has_parents(&sleet_tx) {
                return Err(Error::MissingAncestry);
            }
            sleet_tx.status = TxStatus::Pending;
            self.insert(sleet_tx.clone())?;
            let _ = tx_storage::insert_tx(&self.known_txs, sleet_tx.clone());
            Ok(true)
        } else {
            info!("[{}] received already known transaction {}: {}", "sleet".cyan(), hex::encode(sleet_tx.hash()), sleet_tx.clone());
            Ok(false)
        }
    }

    // Vertices

    pub fn insert(&mut self, tx: Tx) -> Result<()> {
        let cell = tx.cell.clone();
        self.conflict_graph.insert_cell(cell.clone())?;
        let parents = self.remove_accepted_parents(tx.parents.clone());
        self.dag.insert_vx(tx.hash(), parents)?;
        Ok(())
    }

    /// Check if Sleet has all the parents for a transaction
    /// (otherwise the ancestry needs to be fetched
    pub fn has_parents(&self, tx: &Tx) -> bool {
        match self.dag.has_vertices(&tx.parents) {
            Ok(()) => true,
            Err(missing_parents) => missing_parents
                .iter()
                .all(|p| tx_storage::is_accepted_tx(&self.known_txs, p).unwrap_or(false)),
        }
    }

    /// Removes the transactions that already have been accepted, and might not be present
    /// in the DAG at insertion time
    pub fn remove_accepted_parents(&self, mut parents: Vec<TxHash>) -> Vec<TxHash> {
        parents.retain(|p| !tx_storage::is_accepted_tx(&self.known_txs, p).unwrap_or(false));
        parents
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

        // Prefer leaves when selecting parents
        for leaf in leaves.clone() {
            if parents.len() >= p {
                // Found `p` preferred parents.
                break;
            }
            if self.is_strongly_preferred(leaf.clone())? {
                parents.push(leaf.clone());
                accessible.extend(self.dag.dfs(&leaf));
            }
        }

        // If there weren't enough preferred leaves, select parents from their ancestors
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

    /// Recursively reset the confidence counter for a transaction and its ancestry.
    /// Called when a query didn't yield enough votes
    pub fn reset_ancestor_confidence(&mut self, root_txhash: &TxHash) -> Result<()> {
        for tx_hash in self.dag.dfs(root_txhash) {
            self.conflict_graph.reset_count(&tx_hash)?;
        }
        Ok(())
    }

    // Finality

    /// Checks whether the transaction `TxHash` is accepted as final.
    pub fn is_accepted_tx(&self, tx_hash: &TxHash) -> bool {
        // It's a bug if we check a non-existent transaction
        if tx_storage::is_accepted_tx(&self.known_txs, tx_hash).unwrap_or(false) {
            return true;
        }
        if tx_storage::cannot_be_accepted(&self.known_txs, tx_hash).unwrap_or(false) {
            return false;
        }
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
    pub fn remove_conflicts(&mut self, tx: &Tx) -> Result<()> {
        let rejected = self.conflict_graph.accept_cell(tx.cell.clone())?;
        let mut children: VecDeque<TxHash> = VecDeque::new();
        for hash in rejected {
            info!("Rejected {}", hex::encode(hash));
            tx_storage::set_status(&self.known_txs, &hash, TxStatus::Rejected)?;
            let ch = self.dag.remove_vx(&hash)?;
            children.extend(ch.iter());
        }

        // Remove the progeny of conflicting transactions
        while let Some(hash) = children.pop_front() {
            tx_storage::set_status(&self.known_txs, &hash, TxStatus::Removed)?;
            self.conflict_graph.remove_cell(&hash)?;
            // Ignore errors here, as they happen when `children` contains duplicates
            info!("Removed: {}", hex::encode(hash.clone()));
            match self.dag.remove_vx(&hash) {
                Ok(ch) => children.extend(ch.iter()),
                _ => (),
            }
        }

        Ok(())
    }

    // Accepted Frontier

    /// The accepted frontier of the DAG is a depth-first-search on the leaves of the DAG
    /// up to a vertices considered final, collecting all the final nodes.
    pub fn compute_accepted_frontier(&mut self) {
        let mut accepted_frontier = HashSet::new();
        if self.dag.is_empty() {
            self.accepted_frontier = HashSet::new();
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
        self.accepted_frontier = accepted_frontier;
    }

    /// Remove transactions from the dag above the accepted frontier
    pub fn prune_at_accepted_frontier(&mut self) {
        self.compute_accepted_frontier();
        let mut to_be_pruned = HashSet::new();
        for f in self.accepted_frontier.iter() {
            to_be_pruned.extend(self.dag.dfs(f));
        }
        for a in to_be_pruned.iter() {
            if !self.accepted_frontier.contains(a) {
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
            if !tx_storage::is_accepted_tx(&self.known_txs, t).unwrap_or(false)
                && self.is_accepted_memo(t, &mut memo)
            {
                new.push(t.clone());
                let () = self.accepted_txs.insert(t.clone());
                tx_storage::set_status(&self.known_txs, t, TxStatus::Accepted).unwrap();
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

impl Actor for Sleet {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
        debug!("started sleet");
    }

    fn stopping(&mut self, _ctx: &mut Context<Self>) -> actix::Running {
        let _ = self.known_txs.flush();
        actix::Running::Stop
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

            // if !self.live_cells.contains_key(&cell_hash) {
            //     self.live_cells.insert(cell_hash, cell);
            // }
        }
        info!("");
        self.live_cells = BoundedHashMap::new(1000);
        for (k, v) in msg.live_cells.iter() {
            self.live_cells.insert(k.clone(), v.clone());
        }
        self.conflict_graph = ConflictGraph::new(cell_ids_set);

        let mut s: String = format!("<<{}>>\n", "sleet".cyan());
        for (id, (ip, w)) in msg.validators.clone() {
            let id_s = format!("{:?}@{}", id, ip).yellow();
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

    fn handle(&mut self, msg: QueryIncomplete, _ctx: &mut Context<Self>) -> Self::Result {
        self.reset_ancestor_confidence(&msg.tx.hash()).unwrap();
        // Mark as `Queried`, since the transaction won't be queried again
        tx_storage::set_status(&self.known_txs, &msg.tx.hash(), TxStatus::Queried).unwrap();
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
            let () = self.live_cells.insert(msg.tx.cell.hash(), msg.tx.cell.clone());

            // The transaction or some of its ancestors may have become
            // accepted. Check this.
            let new_accepted = self.compute_accepted_txs(&msg.tx.hash());
            if !new_accepted.is_empty() {
                ctx.notify(NewAccepted { tx_hashes: new_accepted });
            }
        } else {
            self.reset_ancestor_confidence(&msg.tx.hash()).unwrap();
        }
        //   if no:  set_chit(tx, 0) -- happens in `insert_vx`
        tx_storage::set_status(&self.known_txs, &msg.tx.hash(), TxStatus::Queried).unwrap();
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

        for tx_hash in msg.tx_hashes.iter().cloned() {
            // At this point we can be sure that the tx is known
            let (_, tx) = tx_storage::get_tx(&self.known_txs, tx_hash).unwrap();

            // Remove conflicting cells and their progeny from the DAG
            match self.remove_conflicts(&tx) {
                Ok(()) => (),
                Err(e) => {
                    info!("Error during removing conflicts: {}", e);
                }
            }
            info!("[{}] transaction is accepted\n{}", "sleet".cyan(), tx.clone());
            cells.push(tx.cell);
        }

        self.prune_at_accepted_frontier();

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

        // Fanout queries to sampled validators
        let send_to_client = self.sender.send(ClientRequest::Fanout {
            peers: validators.clone(),
            request: Request::QueryTx(QueryTx {
                id: self.node_id.clone(),
                ip: self.node_ip.clone(),
                tx: msg.tx.clone(),
            }),
        });

        // Wrap the future so that subsequent chained handlers can access the actor.
        let send_to_client = actix::fut::wrap_future::<_, Self>(send_to_client);

        let update_self = send_to_client.map(move |result, _actor, ctx| {
            match result {
                Ok(ClientResponse::Fanout(acks)) => {
                    // If the length of responses is the same as the length of the sampled ips,
                    // then every peer responded.
                    if acks.len() == validators.len() {
                        Ok(ctx.notify(QueryComplete { tx: msg.tx.clone(), acks }))
                    } else {
                        Ok(ctx.notify(QueryIncomplete { tx: msg.tx.clone(), acks }))
                    }
                }
                Ok(ClientResponse::Oneshot(_)) => panic!("unexpected response"),
                Err(e) => Err(Error::Actix(e)),
            }
        });

        Box::pin(update_self)
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
        let tx_hash = sleet_tx.hash();
        info!("[{}] Generating new transaction: {}\n{}", "sleet".cyan(), hex::encode(tx_hash), sleet_tx);

        match self.on_receive_tx(sleet_tx.clone()) {
            Ok(true) => {
                ctx.notify(FreshTx { tx: sleet_tx });
                GenerateTxAck { cell_hash: Some(msg.cell.hash()) }
            }
            Ok(false) => GenerateTxAck { cell_hash: None },

            Err(e) => {
                error!(
                    "GenerateTx: [{}] Couldn't insert new transaction: {}\n{}:\n {}",
                    "sleet".cyan(),
                    hex::encode(tx_hash),
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
    /// The node's own ID
    pub id: Id,
    /// The node's own listening address, for sending queries back (`GetTxAncestors` in particular)
    pub ip: SocketAddr,
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
                    // TODO we might want this to be a periodic check
                    ctx.notify(CheckPending);
                };

                // We may have accepted or rejected the transaction already when the query comes in
                if tx_storage::is_accepted_tx(&self.known_txs, &tx_hash).unwrap_or(false) {
                    return Box::pin(async move { QueryTxAck { id, tx_hash, outcome: true } });
                }
                if tx_storage::cannot_be_accepted(&self.known_txs, &tx_hash).unwrap_or(false) {
                    return Box::pin(async move { QueryTxAck { id, tx_hash, outcome: false } });
                }

                // FIXME: If we are in the middle of querying this transaction, wait until a
                // decision or a synchronous timebound is reached on attempts.
                let outcome = self.is_strongly_preferred(tx_hash.clone()).unwrap();
                Box::pin(async move { QueryTxAck { id, tx_hash, outcome } })
            }
            Err(Error::MissingAncestry) => {
                info!("[{}] Transaction query: fetching ancestry for {}", "sleet".cyan(), msg.tx);
                let (sender, receiver) = oneshot::channel();
                self.pending_queries.push((msg.tx.clone(), sender));
                // Ask the querying node to send us the ancestors of the queried transaction
                ctx.notify(AskForAncestors { tx_hash: msg.tx.hash(), id: msg.id, ip: msg.ip });
                Box::pin(async move {
                    let timeout = time::sleep(Duration::from_millis(QUERY_RESPONSE_TIMEOUT_MS));
                    tokio::select! {
                        r = receiver => {
                            match r {
                            Ok(outcome) => {
                                // Sleet was able to process the transaction
                                QueryTxAck { id, tx_hash, outcome }
                            },
                            Err(_) => {
                                // This shouldn't happen, Sleet shouldn't drop the sending end
                                error!("Sender for QueryTx outcome errored");
                                QueryTxAck { id, tx_hash, outcome: false }

                            },
                        }
                        },
                        () = timeout => {
                            // Sleet couldn't fetch all ancestors
                            // TODO: we may also respond with a timeout-like message
                            info!("Timeout: Couldn't fetch ancestry for {}", hex::encode(tx_hash));
                            QueryTxAck { id, tx_hash, outcome: false }
                        }
                    }
                })
            }
            Err(e) => {
                error!("QueryTx: [{}] Couldn't insert new transaction:{} \n{}:\n {}", "sleet".cyan(), hex::encode(tx_hash), msg.tx, e);
                Box::pin(async move { QueryTxAck { id, tx_hash, outcome: false } })
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct CheckPending;

impl Handler<CheckPending> for Sleet {
    type Result = ();

    fn handle(&mut self, _msg: CheckPending, ctx: &mut Context<Self>) -> Self::Result {
        let mut remaining = vec![];
        while let Some((tx, sender)) = self.pending_queries.pop() {
            if self.has_parents(&tx) {
                match self.on_receive_tx(tx.clone()) {
                    Ok(is_new) => {
                        if is_new {
                            ctx.notify(FreshTx { tx: tx.clone() });
                        }
                        // TODO: do we need to wait for _our_ query to complete?
                        let outcome = self.is_strongly_preferred(tx.hash()).unwrap();
                        // The receiver might have timed out by now
                        let _ = sender.send(outcome);
                    }
                    Err(e) => {
                        error!(
                            "[{}] Couldn't insert pending transaction\n{}:\n {}",
                            "sleet".cyan(),
                            tx,
                            e
                        );
                        let _ = sender.send(false);
                    }
                }
            } else if sender.is_closed() {
                // The pending query timed out, drop the transaction
                // as we were unable the get its ancestry
                info!("Dropping pending transaction: {}", tx);
            } else {
                remaining.push((tx, sender));
            }
        }
        remaining.reverse();
        self.pending_queries = remaining;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct AskForAncestors {
    pub tx_hash: TxHash,
    pub id: Id,
    pub ip: SocketAddr,
}

impl Handler<AskForAncestors> for Sleet {
    type Result = ResponseActFuture<Self, ()>;

    fn handle(
        &mut self,
        AskForAncestors { tx_hash, id, ip }: AskForAncestors,
        _ctx: &mut Context<Self>,
    ) -> Self::Result {
        self.sender
            .send(ClientRequest::Oneshot {
                id,
                ip,
                request: Request::GetTxAncestors(GetTxAncestors { tx_hash }),
            })
            .into_actor(self)
            .map(|res, act, ctx| match res {
                Ok(ClientResponse::Oneshot(Some(Response::TxAncestors(TxAncestors {
                    ancestors,
                })))) => {
                    for ancestor in ancestors {
                        match act.on_receive_tx(ancestor.clone()) {
                            Ok(is_new) => {
                                if is_new {
                                    // Start querying
                                    ctx.notify(FreshTx { tx: ancestor });
                                };
                            }
                            Err(Error::MissingAncestry) => {
                                // TODO check if this can happen here
                                info!(
                                    "[{}] Couldn't insert transaction (missing ancestry): {}",
                                    "sleet".cyan(),
                                    ancestor
                                );
                            }
                            Err(e) => {
                                error!(
                                    "AskForAncestors: [{}] Couldn't insert new transaction: {}\n{}:\n {}",
                                    "sleet".cyan(),
                                    hex::encode(ancestor.hash()),
                                    ancestor,
                                    e
                                );
                            }
                        }
                    }
                    // Check if there are pending transactions whose ancestry just arrived
                    ctx.notify(CheckPending);
                }
                other => error!("[{}] Unexpected response {:?}", "sleet".cyan(), other),
            })
            .boxed_local()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "TxAncestors")]
pub struct GetTxAncestors {
    tx_hash: TxHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct TxAncestors {
    pub ancestors: Vec<Tx>,
}

impl Handler<GetTxAncestors> for Sleet {
    type Result = TxAncestors;

    fn handle(
        &mut self,
        GetTxAncestors { tx_hash }: GetTxAncestors,
        _ctx: &mut Context<Self>,
    ) -> Self::Result {
        let mut ancestors = vec![];
        let tx_hashes = self.dag.get_ancestors(&tx_hash);
        for hash in tx_hashes {
            let (_, tx) = tx_storage::get_tx(&self.known_txs, hash).unwrap();
            ancestors.push(tx);
        }
        TxAncestors { ancestors }
    }
}

// Message handlers used in testing
pub mod sleet_cell_handlers;
pub mod sleet_status_handler;

// Re-export message types
pub use sleet_cell_handlers::*;

#[cfg(test)]
mod sleet_tests;

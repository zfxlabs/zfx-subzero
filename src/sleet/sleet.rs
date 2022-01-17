use crate::colored::Colorize;
use crate::zfx_id::Id;

use crate::chain::alpha::state::Weight;
use crate::chain::alpha::{self, Transaction, TxHash, UTXOIds};
use crate::client::Fanout;
use crate::graph::{UTXOGraph, DAG};
use crate::hail::AcceptedTransactions;
use crate::protocol::{Request, Response};
use crate::util;

use super::sleet_tx::SleetTx;
use super::{Error, Result};

use tracing::{debug, error, info};

use actix::{Actor, AsyncContext, Context, Handler, Recipient};
use actix::{ActorFutureExt, ResponseActFuture, ResponseFuture};

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

// Parent selection

const NPARENTS: usize = 3;

// Safety parameters

const ALPHA: f64 = 0.5;
const BETA1: u8 = 11;
const BETA2: u8 = 20;

/// Sleet is a consensus bearing `mempool` for transactions conflicting on spent inputs.
pub struct Sleet {
    /// The client used to make external requests.
    sender: Recipient<Fanout>,
    /// Connection to Hail
    hail_recipient: Recipient<AcceptedTransactions>,
    /// The identity of this validator.
    node_id: Id,
    /// The weighted validator set.
    committee: HashMap<Id, (SocketAddr, Weight)>,
    /// The set of all known transactions.
    known_txs: sled::Db,
    /// The set of all queried transactions.
    queried_txs: sled::Db,
    /// The graph of conflicting transactions (potentially multi-input).
    conflict_graph: UTXOGraph,
    /// A vector containing the transactions of the last accepted block.
    txs: HashMap<TxHash, Transaction>,
    /// The map contains transaction already accepted
    accepted_txs: HashSet<TxHash>,
    /// The consensus graph.
    dag: DAG<TxHash>,
}

impl Sleet {
    // Initialisation - FIXME: Temporary databases
    pub fn new(
        sender: Recipient<Fanout>,
        hail_recipient: Recipient<AcceptedTransactions>,
        node_id: Id,
    ) -> Self {
        Sleet {
            sender,
            hail_recipient,
            node_id,
            committee: HashMap::default(),
            known_txs: sled::Config::new().temporary(true).open().unwrap(),
            queried_txs: sled::Config::new().temporary(true).open().unwrap(),
            conflict_graph: UTXOGraph::new(UTXOIds::empty()),
            txs: HashMap::default(),
            accepted_txs: HashSet::default(),
            dag: DAG::new(),
        }
    }

    /// Called for all newly discovered transactions.
    /// Returns `true` if the transaction haven't been encountered before
    fn on_receive_tx(&mut self, sleet_tx: SleetTx) -> Result<bool> {
        let tx = sleet_tx.inner.clone();
        // Skip adding coinbase transactions (block rewards / initial allocations) to the
        // mempool.
        if tx.is_coinbase() {
            Err(Error::InvalidCoinbaseTransaction(tx))
        } else {
            if !alpha::is_known_tx(&self.known_txs, tx.hash()).unwrap() {
                self.insert(sleet_tx.clone())?;
                let _ = alpha::insert_tx(&self.known_txs, tx.clone());
                Ok(true)
            } else {
                // info!("[{}] received already known transaction {}", "sleet".cyan(), tx.clone());
                Ok(false)
            }
        }
    }

    // Vertices

    pub fn insert(&mut self, tx: SleetTx) -> Result<()> {
        let inner_tx = tx.inner.clone();
        self.conflict_graph.insert_tx(inner_tx.clone())?;
        self.dag.insert_vx(inner_tx.hash(), tx.parents.clone())?;
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
        let leaves = self.dag.leaves();
        for leaf in leaves {
            for elt in self.dag.dfs(&leaf) {
                if self.is_strongly_preferred(elt.clone())? {
                    parents.push(elt.clone());
                    if parents.len() >= p {
                        // Found `p` preferred parents.
                        break;
                    } else {
                        // Found a preferred parent for this leaf so skip.
                        continue;
                    }
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
            self.conflict_graph.update_conflict_set(tx_hash, d1, d2)?;
        }
        Ok(())
    }

    // Finality

    /// Checks whether the transaction `TxHash` is accepted as final.
    pub fn is_accepted_tx(&self, tx_hash: &TxHash) -> Result<bool> {
        if self.conflict_graph.is_singleton(tx_hash)?
            && self.conflict_graph.get_confidence(tx_hash)? >= BETA1
        {
            Ok(true)
        } else {
            if self.conflict_graph.get_confidence(tx_hash)? >= BETA2 {
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }

    /// Checks whether the parent of the provided `TxHash` is final - note that we do not
    /// traverse all of the parents of the accepted parent, since a child transaction
    /// cannot be final if its parent is not also final.
    pub fn is_accepted(&self, initial_tx_hash: &TxHash) -> Result<bool> {
        let mut parent_accepted = true;
        match self.dag.get(initial_tx_hash) {
            Some(parents) => {
                for parent in parents.iter() {
                    if !self.is_accepted_tx(&parent)? {
                        parent_accepted = false;
                        break;
                    }
                }
            }
            None => return Err(Error::InvalidTransactionHash(initial_tx_hash.clone())),
        }
        if parent_accepted {
            self.is_accepted_tx(initial_tx_hash)
        } else {
            Ok(false)
        }
    }

    // Accepted Frontier

    /// The accepted frontier of the DAG is a depth-first-search on the leaves of the DAG
    /// up to a vertices considered final, collecting all the final nodes.
    pub fn get_accepted_frontier(&self) -> Result<Vec<TxHash>> {
        if self.dag.is_empty() {
            return Ok(vec![]);
        }
        let mut accepted_frontier = vec![];
        let leaves = self.dag.leaves();
        for leaf in leaves {
            for tx_hash in self.dag.dfs(&leaf) {
                if self.is_accepted(tx_hash)? {
                    accepted_frontier.push(tx_hash.clone());
                    break;
                }
            }
        }
        Ok(accepted_frontier)
    }

    /// Check if a transaction or one of its ancestors have become accepted
    pub fn calculate_accepted_txs(&mut self, tx_hash: &TxHash) -> Result<Vec<TxHash>> {
        let mut new = vec![];
        for t in self.dag.dfs(tx_hash) {
            if !self.accepted_txs.contains(t) && self.is_accepted(t)? {
                new.push(t.clone());
                let _ = self.accepted_txs.insert(t.clone());
            }
        }
        Ok(new)
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
}

// When the committee is initialised in `alpha` or when it comes back online due to a
// `FaultyNetwork` message received in `alpha`, `sleet` is updated with the latest relevant
// chain state.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct LiveCommittee {
    pub validators: HashMap<Id, (SocketAddr, f64)>,
    pub txs: HashMap<TxHash, Transaction>,
}

impl Handler<LiveCommittee> for Sleet {
    type Result = ();

    fn handle(&mut self, msg: LiveCommittee, _ctx: &mut Context<Self>) -> Self::Result {
        // Build the list of available UTXOs
        let txs_len = format!("{:?}", msg.txs.len());
        info!(
            "\n{} received {} transactions containing spendable outputs",
            "[sleet]".cyan(),
            txs_len.cyan()
        );
        let mut utxo_ids_set: UTXOIds = UTXOIds::empty();
        for (tx_hash, tx) in msg.txs.clone() {
            info!("{}", tx.clone());
            let utxo_ids = UTXOIds::from_outputs(tx_hash.clone(), tx.outputs());
            utxo_ids_set = utxo_ids_set.union(&utxo_ids).cloned().collect();
        }
        info!("");
        self.txs = msg.txs.clone();
        self.conflict_graph = UTXOGraph::new(utxo_ids_set);

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
    pub tx: SleetTx,
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
    pub tx: SleetTx,
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
            let _ = self.txs.insert(msg.tx.hash(), msg.tx.inner.clone());

            // The transaction or some of its ancestors may have become
            // accepted. Check this.
            let new_accepted = self.calculate_accepted_txs(&msg.tx.hash());
            match new_accepted {
                Ok(new_accepted) => {
                    if !new_accepted.is_empty() {
                        ctx.notify(NewAccepted { tx_hashes: new_accepted });
                    }
                }
                Err(e) => {
                    // It's a bug if happens
                    panic!("[sleet] Error checking whether transaction is accepted: {}", e);
                }
            }
        }
        //   if no:  set_chit(tx, 0) -- happens in `insert_vx`
        alpha::insert_tx(&self.queried_txs, msg.tx.inner.clone()).unwrap();
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
        let mut txs = vec![];
        for t in msg.tx_hashes.iter() {
            // At this point we can be sure that the tx is known
            let tx = alpha::get_tx(&self.known_txs, t).unwrap().unwrap();
            info!("[{}] transaction is accepted\n{}", "sleet".cyan(), tx);
            txs.push(tx);
        }
        self.hail_recipient.do_send(AcceptedTransactions { txs });
    }
}
// Instead of having an infinite loop as per the paper which receives and processes
// inbound unqueried transactions, we instead use the `Actor` and use `notify` whenever
// a fresh transaction is received - either externally in `GenerateTx` or as an internal
// consensus message via `QueryTx`.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Result<()>")]
pub struct FreshTx {
    pub tx: SleetTx,
}

impl Handler<FreshTx> for Sleet {
    type Result = ResponseActFuture<Self, Result<()>>;

    fn handle(&mut self, msg: FreshTx, _ctx: &mut Context<Self>) -> Self::Result {
        let validators = self.sample(ALPHA).unwrap();
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
#[rtype(result = "TxAck")]
pub struct GetTx {
    pub tx_hash: TxHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct TxAck {
    pub tx: Option<Transaction>,
}

impl Handler<GetTx> for Sleet {
    type Result = TxAck;

    fn handle(&mut self, msg: GetTx, _ctx: &mut Context<Self>) -> Self::Result {
        TxAck { tx: self.txs.get(&msg.tx_hash).map(|x| x.clone()) }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "GenerateTxAck")]
pub struct GenerateTx {
    pub tx: Transaction,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct GenerateTxAck {
    /// hash of applied transaction
    pub tx_hash: Option<TxHash>,
}

impl Handler<GenerateTx> for Sleet {
    type Result = GenerateTxAck;

    fn handle(&mut self, msg: GenerateTx, ctx: &mut Context<Self>) -> Self::Result {
        let parents = self.select_parents(NPARENTS).unwrap();
        let sleet_tx = SleetTx::new(parents, msg.tx.clone());
        info!("[{}] Generating new transaction\n{}", "sleet".cyan(), sleet_tx);

        match self.on_receive_tx(sleet_tx.clone()) {
            Ok(true) => {
                ctx.notify(FreshTx { tx: sleet_tx });
                GenerateTxAck { tx_hash: Some(msg.tx.hash()) }
            }
            Ok(false) => GenerateTxAck { tx_hash: None },

            Err(e) => {
                error!("[{}] Couldn't insert new transaction {}:\n {}", "sleet".cyan(), msg.tx, e);
                GenerateTxAck { tx_hash: None }
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
    pub tx: SleetTx,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct QueryTxAck {
    pub id: Id,
    pub tx_hash: TxHash,
    pub outcome: bool,
}

impl Handler<QueryTx> for Sleet {
    type Result = QueryTxAck;

    fn handle(&mut self, msg: QueryTx, ctx: &mut Context<Self>) -> Self::Result {
        // info!("[{}] Received query for transaction\n{}", "sleet".cyan(), msg.tx.inner.clone());
        match self.on_receive_tx(msg.tx.clone()) {
            Ok(true) => ctx.notify(FreshTx { tx: msg.tx.clone() }),
            Ok(false) => (),
            Err(e) => {
                error!(
                    "[{}] Couldn't insert queried transaction {}: {}",
                    "sleet".cyan(),
                    msg.tx.inner,
                    e
                );
            }
        }
        // FIXME: If we are in the middle of querying this transaction, wait until a
        // decision or a synchronous timebound is reached on attempts.
        match self.is_strongly_preferred(msg.tx.hash()) {
            Ok(outcome) => QueryTxAck { id: self.node_id, tx_hash: msg.tx.hash(), outcome },
            Err(e) => {
                error!("[{}] Missing ancestor of {}: {}", "sleet".cyan(), msg.tx.inner, e);
                // FIXME We're voting against the tx w/o having enough information
                QueryTxAck { id: self.node_id, tx_hash: msg.tx.hash(), outcome: false }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "Transactions")]
pub struct GetTransactions;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct Transactions {
    pub ids: Vec<TxHash>,
}

impl Handler<GetTransactions> for Sleet {
    type Result = Transactions;

    fn handle(&mut self, _msg: GetTransactions, _ctx: &mut Context<Self>) -> Self::Result {
        return Transactions { ids: self.txs.keys().cloned().collect::<Vec<TxHash>>() };
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alpha::tx::{CoinbaseTx, Output, Outputs, Transaction, TransferTx, Tx};
    use ed25519_dalek::Keypair;
    use rand::{rngs::OsRng, CryptoRng};

    fn generate_coinbase(keypair: &Keypair, amount: u64) -> alpha::Transaction {
        let enc = bincode::serialize(&keypair.public).unwrap();
        let pkh = blake3::hash(&enc).as_bytes().clone();
        Transaction::CoinbaseTx(CoinbaseTx::new(Outputs::new(vec![
            Output::new(pkh.clone(), amount),
            Output::new(pkh.clone(), amount + 1),
            Output::new(pkh.clone(), amount + 2),
        ])))
    }

    fn generate_transfer(keypair: &Keypair, from: Transaction, amount: u64) -> alpha::Transaction {
        let enc = bincode::serialize(&keypair.public).unwrap();
        let pkh = blake3::hash(&enc).as_bytes().clone();
        Transaction::TransferTx(TransferTx::new(
            &keypair,
            from.clone(),
            pkh.clone(),
            pkh.clone(),
            amount,
        ))
    }

    struct DummyClient;
    impl Actor for DummyClient {
        type Context = Context<Self>;

        fn started(&mut self, _ctx: &mut Context<Self>) {}
    }

    impl Handler<Fanout> for DummyClient {
        type Result = ResponseFuture<Vec<Response>>;

        fn handle(&mut self, msg: Fanout, ctx: &mut Context<Self>) -> Self::Result {
            Box::pin(async move { vec![] })
        }
    }

    struct HailMock;
    impl Actor for HailMock {
        type Context = Context<Self>;

        fn started(&mut self, _ctx: &mut Context<Self>) {}
    }

    impl Handler<AcceptedTransactions> for HailMock {
        type Result = ();

        fn handle(&mut self, msg: AcceptedTransactions, ctx: &mut Context<Self>) -> Self::Result {
            ()
        }
    }

    #[actix_rt::test]
    async fn test_strongly_preferred() {
        let sender = DummyClient.start();
        let receiver = HailMock.start();

        let mut csprng = OsRng {};
        let root_kp = Keypair::generate(&mut csprng);

        let genesis_tx = generate_coinbase(&root_kp, 1000);
        let genesis_utxo_ids = UTXOIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs());
        let mut sleet = Sleet::new(sender.recipient(), receiver.recipient(), Id::zero());

        // Generate a genesis set of coins
        let stx1 = SleetTx::new(vec![], generate_transfer(&root_kp, genesis_tx.clone(), 1000));
        let stx2 = SleetTx::new(vec![], generate_transfer(&root_kp, genesis_tx.clone(), 1001));
        let stx3 = SleetTx::new(vec![], generate_transfer(&root_kp, genesis_tx.clone(), 1002));

        // Check that parent selection works with an empty DAG.
        let v_empty: Vec<alpha::TxHash> = vec![];
        assert_eq!(sleet.select_parents(3).unwrap(), v_empty.clone());

        // Insert new vertices into the DAG.
        sleet.insert(stx1.clone()).unwrap();
        sleet.insert(stx2.clone()).unwrap();
        sleet.insert(stx3.clone()).unwrap();

        // Coinbase transactions will all conflict, since `tx1` was inserted first it will
        // be the only preferred parent.
        assert_eq!(sleet.select_parents(3).unwrap(), vec![stx1.inner.hash(),]);
    }
}

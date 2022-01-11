use crate::zfx_id::Id;

use crate::colored::Colorize;

use crate::chain::alpha::state::Weight;
use crate::chain::alpha::tx::UTXOId;
use crate::chain::alpha::{self, Transaction, TxHash};
use crate::client;
use crate::graph::DAG;
use crate::protocol::Request;

use super::conflict_map::ConflictMap;
use super::sleet_tx::SleetTx;
use super::{Error, Result};

use rand::seq::SliceRandom;

use tracing::{debug, error, info};

use actix::{Actor, Addr, AsyncContext, Context, Handler, ResponseFuture};

use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::hash::Hash;
use std::net::SocketAddr;

// Parent selection

const NPARENTS: usize = 3;

// Safety parameters

const ALPHA: f64 = 0.5;
const BETA1: u8 = 11;
const BETA2: u8 = 20;

/// Sleet is a consensus bearing `mempool` for transactions conflicting on spent inputs.
pub struct Sleet {
    /// The identity of this validator.
    node_id: Id,
    /// The weighted validator set.
    committee: HashMap<Id, (SocketAddr, Weight)>,
    /// The set of all known transactions.
    known_txs: sled::Db,
    /// The set of all queried transactions.
    queried_txs: sled::Db,
    /// The map of conflicting transactions (potentially multi-input).
    conflict_map: ConflictMap,
    /// A vector containing the transactions of the last accepted block.
    txs: HashMap<TxHash, Transaction>,
    /// The consensus graph.
    dag: DAG<TxHash>,
}

impl Sleet {
    // Initialisation - FIXME: Temporary databases
    pub fn new(node_id: Id) -> Self {
	Sleet {
	    node_id,
	    committee: HashMap::default(),
	    known_txs: sled::Config::new().temporary(true).open().unwrap(),
	    queried_txs: sled::Config::new().temporary(true).open().unwrap(),
	    conflict_map: ConflictMap::new(),
	    txs: HashMap::default(),
	    dag: DAG::new(),
	}
    }

    // Vertices

    pub fn insert(&mut self, tx: SleetTx) ->Result<()>{
        let inner_tx = tx.inner.clone();
        self.conflict_map.insert_tx(inner_tx.clone())?;
        self.dag.insert_vx(inner_tx.hash(), tx.parents.clone())?;
        Ok(())
    }

    // Branch preference

    /// Starts at some vertex and does a depth first search in order to compute whether
    /// the vertex is strongly preferred (by checking whether all its ancestry is
    /// preferred).
    pub fn is_strongly_preferred(&self, tx: TxHash) -> Result<bool> {
        for ancestor in self.dag.dfs(&tx) {
            if !self.conflict_map.is_preferred(ancestor.clone())? {
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
    pub fn update_ancestral_preference(&mut self, tx: Transaction) -> Result<()> {
        for tx_hash in self.dag.dfs(&tx.hash()) {
            // conviction of T vs Pt.pref
            let pref = self.conflict_map.get_preferred(&tx_hash)?;
            let d1 = self.dag.conviction(tx_hash.clone())?;
            let d2 = self.dag.conviction(pref)?;
            // update the conflict set at this tx
            self.conflict_map.update_conflict_set(tx_hash.clone(), d1, d2)?;
        }
        Ok(())
    }

    // Finality

    /// Checks whether the transaction `TxHash` is accepted as final.
    pub fn is_accepted_tx(&self, tx_hash: &TxHash) -> Result<bool> {
        if self.conflict_map.is_singleton(tx_hash)?
            && self.conflict_map.get_confidence(tx_hash)? >= BETA1
        {
            Ok(true)
        } else {
            if self.conflict_map.get_confidence(tx_hash)? >= BETA2 {
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }

    /// Checks whether the parent of the provided `TxHash` is final - note that we do not
    /// traverse all of the parents of the accepted parent, since a child transaction
    /// cannot be final if its parent is not also final.
    pub fn is_accepted(&self, initial_tx_hash: TxHash) -> Result<bool> {
        let mut parent_accepted = true;
        match self.dag.get(&initial_tx_hash) {
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
            self.is_accepted_tx(&initial_tx_hash)
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
                if self.is_accepted(tx_hash.clone())? {
                    accepted_frontier.push(tx_hash.clone());
                    break;
                }
            }
        }
        Ok(accepted_frontier)
    }

    // Weighted sampling

    pub fn sample(&self, minimum_weight: Weight) -> Result<Vec<(Id, SocketAddr)>> {
        let mut validators = vec![];
        for (id, (ip, w)) in self.committee.iter() {
            validators.push((id.clone(), ip.clone(), w.clone()));
        }
        sample_weighted(minimum_weight, validators)
    }

    /// Checks whether a transactions inputs spends valid outputs. If two transactions
    /// spend the same outputs in the mempool, this is resolved via the conflict map -
    /// it is not an error to receive two conflicting transactions.
    pub fn spends_valid_utxos(&self, tx: Transaction) -> bool {
        for input in tx.inputs().iter() {
            match self.txs.get(&input.source) {
                Some(unspent_tx) => {
                    // FIXME: Better verification.
                    let utxos = unspent_tx.outputs();
                    if input.i as usize >= utxos.len() {
                        error!("invalid transaction index");
                        return false;
                    }
                }
                None => {
                    error!("invalid input source");
                    return false;
                }
            }
        }
        true
    }
}

#[inline]
fn sample_weighted(
    min_w: Weight,
    mut validators: Vec<(Id, SocketAddr, Weight)>,
) -> Result<Vec<(Id, SocketAddr)>> {
    let mut rng = rand::thread_rng();
    validators.shuffle(&mut rng);
    let mut sample = vec![];
    let mut w = 0.0;
    for (id, ip, w_v) in validators {
        if w >= min_w {
            break;
        }
        sample.push((id, ip));
        w += w_v;
    }
    if w < min_w {
        Err(Error::InsufficientWeight)
    } else {
        Ok(sample)
    }
}

#[inline]
pub fn sum_outcomes(outcomes: Vec<(Id, Weight, bool)>) -> f64 {
    outcomes
        .iter()
        .fold(0.0, |acc, (_id, weight, result)| if *result { acc + *weight } else { acc })
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
        for (_tx_hash, tx) in msg.txs.clone() {
            info!("{}", tx.clone());
        }
        info!("");
        self.txs = msg.txs.clone();

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

// Instead of having an infinite loop as per the paper which receives and processes
// inbound unqueried transactions, we instead use the `Actor` and use `notify` whenever
// a fresh transaction is received - either externally in `ReceiveTx` or as an internal
// consensus message via `QueryTx`.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct FreshTx {
    pub tx: Transaction,
}

impl Handler<FreshTx> for Sleet {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: FreshTx, ctx: &mut Context<Self>) -> Self::Result {
	let validators = self.sample(ALPHA).unwrap();
	info!("[{}] sampled {:?}", "sleet".cyan(), validators.clone());
	let mut validator_ips = vec![];
	for (_, ip) in validators.iter() {
	    validator_ips.push(ip.clone());
	}
	Box::pin(async move {
	    let tx = msg.tx;

	    // Fanout queries to sampled validators
	    let v = client::fanout(validator_ips.clone(), Request::QueryTx(QueryTx {
		tx: tx.clone(),
	    })).await;

	    // If the length of responses is the same as the length of the sampled ips, then
	    // every peer responded.
	    if v.len() == validator_ips.len() {
		// Otherwise check if `k` * `alpha` > `quiescent_point`
		//   if yes: set_chit(tx, 1), update ancestral preferences
		//   if no:  set_chit(tx, 0)

		// Add the transaction to the queried set
		// ctx.notify(QueryComplete { tx: tx.clone() })
		// alpha::insert_tx(&self.queried_txs, tx.clone()).unwrap();
	    } else {
		// FIXME: If `v` is smaller than the length of the sampled `validator_ips`
		// then it means this query must be re-attempted later (synchronous
		// timebound condition) -- the transaction cannot be marked as queried in
		// this case since it is this validators faulty connection which caused
		// the error.
	    }
	})
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
    pub tx: Transaction,
}

impl Handler<GetTx> for Sleet {
    type Result = TxAck;

    fn handle(&mut self, msg: GetTx, _ctx: &mut Context<Self>) -> Self::Result {
        let tx = self.txs.get(&msg.tx_hash).unwrap();
        TxAck { tx: tx.clone() }
    }
}

// Receiving transactions. The only difference between receiving transactions and receiving
// a transaction query is that any client should be able to send `sleet` a `ReceiveTx`
// message, whereas only network validators should be able to perform a `QueryTx`.
//
// Otherwise the functionality is identical but `QueryTx` returns a consensus response -
// whether the transaction is strongly preferred or not.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "ReceiveTxAck")]
pub struct ReceiveTx {
    pub tx: Transaction,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct ReceiveTxAck;

impl Handler<ReceiveTx> for Sleet {
    type Result = ReceiveTxAck;

    fn handle(&mut self, msg: ReceiveTx, ctx: &mut Context<Self>) -> Self::Result {
        let tx = msg.tx.clone();
        // Skip adding coinbase transactions (block rewards / initial allocations) to the
        // mempool.
        if tx.is_coinbase() {
            // FIXME: receiving a coinbase transaction should result in an error
            ReceiveTxAck {}
        } else {
            if !alpha::is_known_tx(&self.known_txs, tx.hash()).unwrap() {
                info!("[{}] received new transaction {}", "sleet".cyan(), tx.clone());
                if self.spends_valid_utxos(tx.clone()) {
                    let parents = self.select_parents(NPARENTS).unwrap();
                    self.insert(SleetTx::new(parents, tx.clone())).unwrap();
                    alpha::insert_tx(&self.known_txs, tx.clone()).unwrap();
                    ctx.notify(FreshTx { tx: tx.clone() });
                } else {
                    // FIXME: better error handling
                    error!("invalid transaction");
                }
            }
            ReceiveTxAck {}
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "QueryTxAck")]
pub struct QueryTx {
    pub tx: Transaction,
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
	let tx = msg.tx.clone();
	// Skip adding coinbase transactions (block rewards / initial allocations) to the
	// mempool.
	if tx.is_coinbase() {
	    // FIXME: querying about a coinbase should result in an error
	    QueryTxAck { id: self.node_id, tx_hash: tx.hash(), outcome: false }
	} else {
	    if !alpha::is_known_tx(&self.known_txs, tx.hash()).unwrap() {
		info!("sleet: received new transaction {:?}", tx.clone());
		if self.spends_valid_utxos(tx.clone()) {
		    let parents = self.select_parents(NPARENTS).unwrap();
		    self.insert(SleetTx::new(parents, tx.clone()));
		    alpha::insert_tx(&self.known_txs, tx.clone()).unwrap();
		    ctx.notify(FreshTx { tx: tx.clone() });
		} else {
		    error!("invalid transaction");
		}
	    }
	    // FIXME: If we are in the middle of querying this transaction, wait until a
	    // decision or a synchronous timebound is reached on attempts.
	    let outcome = self.is_strongly_preferred(msg.tx.hash()).unwrap();
	    QueryTxAck { id: self.node_id, tx_hash: msg.tx.hash(), outcome }
	}
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alpha::tx::{CoinbaseTx, Transaction, Tx};
    use ed25519_dalek::Keypair;
    use rand::{rngs::OsRng, CryptoRng};

    fn generate_coinbase(keypair: &Keypair, amount: u64) -> alpha::Transaction {
        let enc = bincode::serialize(&keypair.public).unwrap();
        let pkh = blake3::hash(&enc);
        let tx = Tx::coinbase(pkh.as_bytes().clone(), amount);
        Transaction::CoinbaseTx(CoinbaseTx { tx })
    }

    #[actix_rt::test]
    async fn test_strongly_preferred() {
        let mut sleet = Sleet::new();

        let mut csprng = OsRng {};
        let root_kp = Keypair::generate(&mut csprng);

        // Generate a genesis set of coins
        let stx1 = SleetTx::new(vec![], generate_coinbase(&root_kp, 1000));
        let stx2 = SleetTx::new(vec![], generate_coinbase(&root_kp, 1001));
        let stx3 = SleetTx::new(vec![], generate_coinbase(&root_kp, 1002));

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

    #[actix_rt::test]
    async fn test_sampling_insufficient_stake() {
        let dummy_ip: SocketAddr = "0.0.0.0:1111".parse().unwrap();

        let empty = vec![];
        match sample_weighted(0.66, empty) {
            Err(Error::InsufficientWeight) => (),
            x => panic!("unexpected: {:?}", x),
        }

        let not_enough = vec![(Id::one(), dummy_ip, 0.1), (Id::two(), dummy_ip, 0.1)];
        match sample_weighted(0.66, not_enough) {
            Err(Error::InsufficientWeight) => (),
            x => panic!("unexpected: {:?}", x),
        }
    }

    #[actix_rt::test]
    async fn test_sampling() {
        let dummy_ip: SocketAddr = "0.0.0.0:1111".parse().unwrap();

        let v = vec![(Id::one(), dummy_ip, 0.7)];
        match sample_weighted(0.66, v) {
            Ok(v) => assert!(v == vec![(Id::one(), dummy_ip)]),
            x => panic!("unexpected: {:?}", x),
        }

        let v = vec![(Id::one(), dummy_ip, 0.6), (Id::two(), dummy_ip, 0.1)];
        match sample_weighted(0.66, v) {
            Ok(v) => assert!(v.len() == 2),
            x => panic!("unexpected: {:?}", x),
        }

        let v = vec![
            (Id::one(), dummy_ip, 0.6),
            (Id::two(), dummy_ip, 0.1),
            (Id::zero(), dummy_ip, 0.1),
        ];
        match sample_weighted(0.66, v) {
            Ok(v) => assert!(v.len() >= 2 && v.len() <= 3),
            x => panic!("unexpected: {:?}", x),
        }
    }

    #[actix_rt::test]
    async fn test_sum_outcomes() {
        let zid = Id::zero();
        let empty = vec![];
        assert_eq!(0.0, sum_outcomes(empty));

        let one_true = vec![(zid, 0.66, true)];
        assert_eq!(0.66, sum_outcomes(one_true));

        let one_false = vec![(zid, 0.66, false)];
        assert_eq!(0.0, sum_outcomes(one_false));

        let true_false =
            vec![(zid, 0.1, false), (zid, 0.1, true), (zid, 0.1, false), (zid, 0.1, true)];
        assert_eq!(0.2, sum_outcomes(true_false));
    }
}

use zfx_id::Id;

use crate::graph::DAG;
use crate::chain::alpha::{self, Transaction, TxHash};
use crate::chain::alpha::tx::UTXOId;
use crate::chain::alpha::state::Weight;
use crate::util;

use super::{Result, Error};
use super::conflict_map::ConflictMap;
use super::sleet_tx::SleetTx;

use rand::seq::SliceRandom;

use tracing::{debug, info, error};

use actix::{Actor, Context, Handler, Addr};

use std::collections::{HashSet, HashMap, hash_map::Entry};
use std::hash::Hash;

// Parent selection

const NPARENTS: usize = 3;

// Security parameters

const BETA1: u8 = 11;
const BETA2: u8 = 20;

/// Sleet is a consensus bearing `mempool` for transactions conflicting on spent inputs.
pub struct Sleet {
    /// The weighted validator set.
    validators: Vec<(Id, Weight)>,
    /// The set of all known transactions.
    known_txs: sled::Db,
    /// The set of all queried transactions.
    queried_txs: sled::Db,
    /// The map of conflicting transactions (potentially multi-input).
    conflict_map: ConflictMap,
    /// A hashset containing the currently spendable UTXO ids.
    utxo_ids: HashSet<UTXOId>,
    /// The consensus graph.
    dag: DAG<TxHash>,
}

impl Sleet {

    // Initialisation - FIXME: Temporary databases
    pub fn new() -> Self {
	Sleet {
	    validators: vec![],
	    known_txs: sled::Config::new().temporary(true).open().unwrap(),
	    queried_txs: sled::Config::new().temporary(true).open().unwrap(),
	    conflict_map: ConflictMap::new(),
	    utxo_ids: HashSet::new(),
	    dag: DAG::new(),
	}
    }

    // Vertices

    pub fn insert(&mut self, tx: SleetTx) {
	let inner_tx = tx.inner.clone();
	self.conflict_map.insert_tx(inner_tx.clone());
	self.dag.insert_vx(inner_tx.hash(), tx.parents.clone());
    }

    // Branch preference

    /// Starts at some vertex and does a depth first search in order to compute whether
    /// the vertex is strongly preferred (by checking whether all its ancestry is
    /// preferred).
    pub fn is_strongly_preferred(&self, tx: TxHash) -> Result<bool> {
        for ancestor in self.dag.dfs(tx) {
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
            for elt in self.dag.dfs(leaf) {
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
        let ancestors: Vec<_> = self.dag.dfs(tx.hash()).collect();
        for tx_hash in ancestors {
            // conviction of T vs Pt.pref
            let pref = self.conflict_map.get_preferred(&tx_hash)?;
            let d1 = self.dag.conviction(tx_hash.clone())?;
            let d2 = self.dag.conviction(pref)?;
            // update the conflict set at this tx
            self.conflict_map.update_conflict_set(tx_hash.clone(), d1, d2);
        }
        Ok(())
    }

    // Finality

    /// Checks whether the transaction `TxHash` is accepted as final.
    pub fn is_accepted_tx(&self, tx_hash: &TxHash) -> Result<bool> {
	if self.conflict_map.is_singleton(tx_hash)? &&
	    self.conflict_map.get_confidence(tx_hash)? >= BETA1 {
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
	let mut parent_accepted = false;
	match self.dag.get(&initial_tx_hash) {
	    Some(parents) => {
		for parent in parents.iter() {
		    if self.is_accepted_tx(&parent)? {
			parent_accepted = true;
			break;
		    }
		}
	    },
	    None =>
		return Err(Error::InvalidTransactionHash(initial_tx_hash.clone())),
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
	    for tx_hash in self.dag.dfs(leaf) {
		if self.is_accepted(tx_hash)? {
		    accepted_frontier.push(tx_hash.clone());
		    break;
		} else {
		    break;
		}
	    }
	}
	Ok(accepted_frontier)
    }

    // Weighted sampling

    pub fn sample(&self, weight: Weight) -> Result<Vec<Id>> {
        weighted_sample(weight, self.validators.clone())
    }
}

#[inline]
fn weighted_sample(weight: Weight, mut validators: Vec<(Id, Weight)>) -> Result<Vec<Id>> {
    let mut rng = rand::thread_rng();
    validators.shuffle(&mut rng);
    let mut sample = vec![];
    let mut w = 0.0;
    for (id, w_v) in validators {
        if w >= weight {
            break;
        }
        sample.push(id);
        w += w_v;
    }
    if w < weight {
        Err(Error::InsufficientWeight)
    } else {
        Ok(sample)
    }
}

impl Actor for Sleet {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
	debug!("started Sleet");
    }
}

// When the committee is initialised in `alpha` or when it comes back online due to a
// `FaultyNetwork` message received in `alpha`, `sleet` is updated with the latest relevant
// chain state.

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct LiveCommittee {
    pub validators: Vec<(Id, u64)>,
    pub initial_supply: u64,
    pub utxo_ids: HashSet<UTXOId>,
}

impl Handler<LiveCommittee> for Sleet {
    type Result = ();

    fn handle(&mut self, msg: LiveCommittee, _ctx: &mut Context<Self>) -> Self::Result {
	let spendable = msg.utxo_ids.clone();
	info!("sleet received {:?} spendable outputs", spendable);
	let mut weighted_validators = vec![];
	for (id, amount) in msg.validators {
	    let v_w = util::percent_of(amount, msg.initial_supply);
	    weighted_validators.push((id.clone(), v_w));
	}

	// Update the list of UTXO Ids / weighted validator set
	self.utxo_ids = msg.utxo_ids.clone();
	self.validators = weighted_validators.clone();
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

    fn handle(&mut self, msg: ReceiveTx, _ctx: &mut Context<Self>) -> Self::Result {
	let tx = msg.tx.clone();
	// Skip adding coinbase transactions (block rewards / initial allocations) to the
	// mempool.
	if tx.is_coinbase() {
	    // FIXME: receiving a coinbase transaction should result in an error
	    ReceiveTxAck{}
	} else {
	    if !alpha::is_known_tx(&self.known_txs, tx.hash()).unwrap() {
		info!("sleet: received new transaction {:?}", tx.clone());
		// if spends_valid_utxo(msg.tx.clone()) {
		let parents = self.select_parents(NPARENTS).unwrap();
		self.insert(SleetTx::new(parents, tx.clone()));
		alpha::insert_tx(&self.known_txs, tx.clone()).unwrap();
		// }
	    }
	    ReceiveTxAck{}
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
    pub tx_hash: TxHash,
    pub outcome: bool,
}

impl Handler<QueryTx> for Sleet {
    type Result = QueryTxAck;

    fn handle(&mut self, msg: QueryTx, _ctx: &mut Context<Self>) -> Self::Result {
	let tx = msg.tx.clone();
	// Skip adding coinbase transactions (block rewards / initial allocations) to the
	// mempool.
	if tx.is_coinbase() {
	    // FIXME: querying about a coinbase should result in an error
	    QueryTxAck { tx_hash: tx.hash(), outcome: false }
	} else {
	    if !alpha::is_known_tx(&self.known_txs, msg.tx.hash()).unwrap() {
		info!("sleet: received new transaction {:?}", msg.tx.clone());
		let parents = self.select_parents(NPARENTS).unwrap();
		self.insert(SleetTx::new(parents, msg.tx.clone()));
	    }
	    let outcome = self.is_strongly_preferred(msg.tx.hash()).unwrap();
	    QueryTxAck { tx_hash: msg.tx.hash(), outcome }
	}
    }
}

// Runs the main consensus loop
// pub async fn run() {
//     loop {
// 	// Find an unqueried transaction.
//
// 	// Sample `k` random peers from the live committee.
//
// 	// Query the peers.
//
// 	// If `k` * `alpha` > `quiescent_point`:
// 	//   -> chit = 1, update ancestors
// 	// Otherwise:
// 	//   -> chit = 0
//
// 	// Add the transaction to the queried set.
//     }
// }


#[cfg(test)]
mod test {
    use super::*;
    use rand::{CryptoRng, rngs::OsRng};
    use ed25519_dalek::Keypair;

    // fn generate_coinbase(keypair: Keypair, amount: u64) -> alpha::Transaction {
    // 	let enc = bincode::serialize(&keypair.public).unwrap();
    // 	let pkh = blake3::hash(&enc);
    // 	alpha::Tx::coinbase(pkh.as_bytes().clone(), amount)
    // }

    // #[actix_rt::test]
    // async fn test_strongly_preferred() {
    // 	let mut sleet = Sleet::new();

    // 	let mut csprng = OsRng{};
    // 	let root_kp = Keypair::generate(&mut csprng);

    // 	// Generate a genesis set of coins
    // 	let tx1 = generate_coinbase(root_kp, 1000);

    // 	let stx1 = SleetTx::new(vec![], tx1.clone());
    // 	let stx2 = SleetTx::new(vec![], tx1.clone());
    // 	let stx3 = SleetTx::new(vec![], tx1.clone());

    // 	// Check that parent selection works with an empty DAG.
    // 	let v_empty: Vec<alpha::TxHash> = vec![];
    // 	assert_eq!(sleet.select_parents(3).unwrap(), v_empty.clone());

    // 	// Insert new vertices into the DAG.
    // 	sleet.insert(stx1.clone());
    // 	sleet.insert(stx2.clone());
    // 	sleet.insert(stx3.clone());

    // 	// Coinbase transactions will all conflict, since `tx1` was inserted first it will
    // 	// be the only preferred parent.
    // 	assert_eq!(sleet.select_parents(3).unwrap(), vec![
    // 	    tx1.clone().hash(),
    // 	]);
    // }

    #[actix_rt::test]
    async fn test_sampling_insufficient_stake() {
        let empty = vec![];
       match weighted_sample(0.66, empty) {
            Err(Error::InsufficientWeight) => (),
            x => panic!("unexpected: {:?}", x)
        }
        let not_enough = vec![(Id::one(), 0.1), (Id::two(), 0.1)];
        match weighted_sample(0.66, not_enough) {
            Err(Error::InsufficientWeight) => (),
            x => panic!("unexpected: {:?}", x)
        }
    }
    #[actix_rt::test]
    async fn test_sampling() {
        let v = vec![(Id::one(), 0.7)];
        match weighted_sample(0.66, v) {
            Ok(v) => assert!(v == vec![Id::one()]),
            x => panic!("unexpected: {:?}", x),
        }

        let v = vec![(Id::one(), 0.6), (Id::two(), 0.1)];
        match weighted_sample(0.66, v) {
            Ok(v) => assert!(v.len() == 2),
            x => panic!("unexpected: {:?}", x),
        }

        let v = vec![(Id::one(), 0.6), (Id::two(), 0.1), (Id::zero(), 0.1)];
        match weighted_sample(0.66, v) {
            Ok(v) => assert!(v.len() >= 2 && v.len() <= 3),
            x => panic!("unexpected: {:?}", x),
        }
    }
}

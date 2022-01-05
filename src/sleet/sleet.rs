use crate::graph::DAG;
use crate::chain::alpha;

use super::Result;
use super::conflict_map::ConflictMap;
use super::tx::SleetTx;

use tracing::{debug, info, error};

use actix::{Actor, Context, Handler, Addr};

use std::collections::{HashMap, hash_map::Entry};
use std::hash::Hash;

// Security parameters

const BETA1: usize = 11;
const BETA2: usize = 20;

// Sleet

pub struct Sleet {
    known_txs: sled::Db,
    queried_txs: sled::Db,
    conflict_map: ConflictMap,
    dag: DAG<alpha::TxHash>,
}

impl Sleet {

    // Initialisation - FIXME: Temporary databases
    pub fn new() -> Self {
	Sleet {
	    known_txs: sled::Config::new().temporary(true).open().unwrap(),
	    queried_txs: sled::Config::new().temporary(true).open().unwrap(),
	    conflict_map: ConflictMap::new(),
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
    pub fn is_strongly_preferred(&self, tx: alpha::TxHash) -> Result<bool> {
	let mut visited: HashMap<alpha::TxHash, bool> = HashMap::default();
	let mut stack = vec![];
	stack.push(tx.clone());
	    
	loop {
	    if stack.len() == 0 {
		break;
	    }
	    let elt = stack.pop().unwrap();
	    match visited.entry(elt.clone()) {
		Entry::Occupied(_) => (),
		Entry::Vacant(mut v) => {
		    let _ = v.insert(true);
		    // Instead of saving the node here we check if it is strongly preferred
		    // along the dfs and return false if not.
		    if !self.conflict_map.is_preferred(elt.clone())? {
			return Ok(false);
		    }
		},
	    }
	    let adj = self.dag.get(&elt).unwrap();
	    for edge in adj.iter().cloned() {
		match visited.entry(edge.clone()) {
		    Entry::Occupied(_) =>
			(),
		    Entry::Vacant(_) =>
			stack.push(edge),
		}
	    }
	}
	// All nodes have been visited along the DFS - the node is strongly preferred.
	Ok(true)
    }

    // Adaptive Parent Selection

    /// Starts at the live edges (the leaf nodes) of the `DAG` and does a depth first
    /// search until `p` preferrential parents are accumulated (or none if there are
    /// none).
    pub fn select_parents(&self, p: usize) -> Result<Vec<alpha::TxHash>> {
	if self.dag.len() == 0 {
	    Ok(vec![])
	} else {
	    let mut parents = vec![];
	    let leaves = self.dag.leaves();
	    for leaf in leaves.iter() {
		let mut visited: HashMap<alpha::TxHash, bool> = HashMap::default();
		let mut stack = vec![];
		stack.push(leaf.clone());

		loop {
		    if stack.len() == 0 {
			break;
		    }
		    let elt = stack.pop().unwrap();
		    match visited.entry(elt.clone()) {
			Entry::Occupied(_) => (),
			Entry::Vacant(mut v) => {
			    if self.is_strongly_preferred(elt.clone())? {
				parents.push(elt.clone());
				v.insert(true);
				if parents.len() >= p {
				    // Found `p` preferred parents.
				    break;
				} else {
				    // Found a preferred parent for this leaf so skip.
				    continue;
				}
			    }
			},
		    }
		    let adj = self.dag.get(&elt).unwrap();
		    for edge in adj.iter().cloned() {
			match visited.entry(edge.clone()) {
			    Entry::Occupied(_) =>
				(),
			    Entry::Vacant(_) =>
				stack.push(edge),
			}
		    }
		}
	    }
	    Ok(parents)
	}
    }

    // Ancestral Preference

    // The ancestral update updates the preferred path through the DAG every time a new
    // vertex is added. 
    // pub fn update_ancestors(&mut self) { }

    // Live Frontier

    // The live frontier of the DAG is a depth-first-search on the leaves of the DAG
    // up to a vertices considered final.
}

impl Actor for Sleet {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
	debug!("started Sleet");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "()")]
pub struct ReceiveTx {
    pub tx: alpha::Tx,
}

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct ReceiveTxAck {
    pub tx_hash: alpha::TxHash,
    pub outcome: bool,
}

// Receiving transactions

// pub fn on_receive(&mut self, t: Tx) {
// 	if !state::exists(self.known_txs, &t) {
//      // Check whether the inputs conflict with other inputs
// 	    self.insert(t);
// 	}
// }

impl Handler<ReceiveTx> for Sleet {
    type Result = ();

    fn handle(&mut self, msg: ReceiveTx, _ctx: &mut Context<Self>) -> Self::Result {
	info!("sleet: received {:?}", msg.clone());
	()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[rtype(result = "QueryTxAck")]
pub struct QueryTx;

#[derive(Debug, Clone, Serialize, Deserialize, MessageResponse)]
pub struct QueryTxAck;

impl Handler<QueryTx> for Sleet {
    type Result = QueryTxAck;

    fn handle(&mut self, msg: QueryTx, _ctx: &mut Context<Self>) -> Self::Result {
	QueryTxAck {}
    }
}

// Runs the main consensus loop
// pub async fn run() {
//     loop {
// 	// Receive an unqueried transaction.
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

    fn generate_coinbase(keypair: Keypair, amount: u64) -> alpha::Tx {
	let enc = bincode::serialize(&keypair.public).unwrap();
	let pkh = blake3::hash(&enc);
	alpha::Tx::coinbase(pkh.as_bytes().clone(), amount)
    }

    #[actix_rt::test]
    async fn test_strongly_preferred() {
	let mut sleet = Sleet::new();

	let mut csprng = OsRng{};
	let root_kp = Keypair::generate(&mut csprng);

	// Generate a genesis set of coins
	let tx1 = generate_coinbase(root_kp, 1000);

	let stx1 = SleetTx::new(vec![], tx1.clone());
	let stx2 = SleetTx::new(vec![], tx1.clone());
	let stx3 = SleetTx::new(vec![], tx1.clone());

	// Check that parent selection works with an empty DAG.
	let v_empty: Vec<alpha::TxHash> = vec![];
	assert_eq!(sleet.select_parents(3).unwrap(), v_empty.clone());

	// Insert new vertices into the DAG.
	sleet.insert(stx1.clone());
	sleet.insert(stx2.clone());
	sleet.insert(stx3.clone());

	// Coinbase transactions will all conflict, since `tx1` was inserted first it will
	// be the only preferred parent.
	assert_eq!(sleet.select_parents(3).unwrap(), vec![
	    tx1.clone().hash(),
	]);
    }
}

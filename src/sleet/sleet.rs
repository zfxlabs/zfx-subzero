use crate::graph::DAG;
use crate::chain::alpha::Transaction;

use super::Result;
use super::conflict_map::ConflictMap;
use super::tx::{Tx, TxHash};

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
    dag: DAG<TxHash>,
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

    pub fn insert(&mut self, t: Tx) {
	let h = t.hash();
     	self.conflict_map.insert(h);
	self.dag.insert_vx(h, t.parents);
    }
    
    // Branch preference

    /// Starts at some vertex and does a depth first search in order to compute whether
    /// the vertex is strongly preferred (by checking whether all its ancestry is
    /// preferred).
    pub fn is_strongly_preferred(&self, t: TxHash) -> Result<bool> {
	let mut visited: HashMap<TxHash, bool> = HashMap::default();
	let mut stack = vec![];
	stack.push(t.clone());
	    
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
    pub fn select_parents(&self, p: usize) -> Result<Vec<TxHash>> {
	if self.dag.len() == 0 {
	    Ok(vec![])
	} else {
	    let mut parents = vec![];
	    let leaves = self.dag.leaves();
	    for leaf in leaves.iter() {
		let mut visited: HashMap<TxHash, bool> = HashMap::default();
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

    // Receiving transactions

    // pub fn on_receive(&mut self, t: Tx) {
    // 	if !state::exists(self.known_txs, &t) {
    //      // Check whether the inputs conflict with other inputs
    // 	    self.insert(t);
    // 	}
    // }

    // Spending transactions
}

#[cfg(test)]
mod test {
    use super::*;
    use rand::{CryptoRng, rngs::OsRng};
    use ed25519_dalek::Keypair;

    fn generate_coinbase(amount: u64) -> Transaction {
	let mut csprng = OsRng{};
	let kp = Keypair::generate(&mut csprng);
	let enc = bincode::serialize(&kp.public).unwrap();
	let pkh = blake3::hash(&enc);
	Transaction::coinbase(pkh.as_bytes().clone(), amount)
    }

    #[actix_rt::test]
    async fn test_strongly_preferred() {
	let mut sleet = Sleet::new();

	let tx1 = Tx::new(vec![], generate_coinbase(1000));
	let tx2 = Tx::new(vec![], generate_coinbase(1000));
	let tx3 = Tx::new(vec![], generate_coinbase(1000));

	// Check that parent selection works with an empty DAG.
	let v_empty: Vec<TxHash> = vec![];
	assert_eq!(sleet.select_parents(3).unwrap(), v_empty.clone());

	// Insert new vertices into the DAG.
	sleet.insert(tx1);
	sleet.insert(tx2);
	sleet.insert(tx3);

	// assert_eq!(sleet.select_parents(3).unwrap(), vec![tx1,tx2,tx3]);
    }
}

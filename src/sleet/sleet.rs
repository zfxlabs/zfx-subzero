use crate::graph::DAG;

use super::Result;
use super::tx::{Tx, TxHash};
use super::conflict_map::ConflictMap;

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

    /// Starts at the live edges (the leaf nodes) of the `DAG` and does a depth first search
    /// until `p` strongly preferred nodes are accumulated. Strongly preferred nodes are
    /// leaf nodes whose parents are all preferred.
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
		    if !self.conflict_map.is_strongly_preferred(elt.clone())? {
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
    /// search until `p` preferrential parents are accumulated (or none if there are none).
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

    // Chit Accumulation

    // Finding the confidence of a vertex entails summing the progeny of a vertex. 

    // Ancestral Preference

    // Receiving transactions

    // pub fn on_receive(&mut self, t: Tx) {
    // 	if !state::exists(self.known_txs, &t) {
    // 	    self.insert(t);
    // 	}
    // }

    // Live Frontier

    // The live frontier of the DAG is a depth-first-search on the leaves of the DAG
    // up to a vertices considered final.
}

#[cfg(test)]
mod test {
    use super::*;

    #[actix_rt::test]
    async fn test_sleet() {
	let mut sleet = Sleet::new();

	let tx1 = Tx::new(vec![], vec![1]);
	let tx2 = Tx::new(vec![], vec![2]);

	sleet.insert(tx1);
	sleet.insert(tx2);
    }
}

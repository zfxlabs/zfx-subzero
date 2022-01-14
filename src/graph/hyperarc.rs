use super::{Result, Error};

use crate::chain::alpha::tx::{Tx, Inputs, Outputs, Input};

use std::hash::Hash;
use std::cmp::Ord;

#[derive(Debug)]
pub struct Hyperarc {
    arcs: Vec<(Inputs<Input>, Vec<Tx>, Tx)>, // last entry is `pref`
}

impl Hyperarc
{
    pub fn new() -> Self {
	Hyperarc { arcs: vec![] }
    }

    pub fn get(&self, inputs: &Inputs<Input>) -> Option<(Vec<Tx>, Tx)> {
	// A vector containing the transactions which conflict on the supplied inputs
	let mut v = vec![];
	// The preference for a particular transaction
	let mut p = None;
	for i in 0..self.arcs.len() {
	    // If the inputs intersect then we have a conflict
	    if !self.arcs[i].0.is_disjoint(inputs) {
		// Checks the original inputs of the transaction for intersection, so
		// that we do not include conflicts which are non-conflicting with
		// the supplied inputs and pushes the transaction if it intersects.
		for tx in self.arcs[i].1.iter() {
		    if !tx.inputs.is_disjoint(inputs) {
			v.push(tx.clone());
			// If the preference has not yet been set, then we set the
			// preference to te first entry. Note that this assumes that
			// the arcs are sorted.
			if p.is_none() {
			    p = Some(self.arcs[i].2.clone());
			}
		    }
		}
	    }
	}
	if v.len() > 0 {
	    Some((v, p.unwrap()))
	} else {
	    None
	}
    }

    pub fn update(&mut self, tx: Tx) {
	let mut exact_match = false;
	for i in 0..self.arcs.len() {
	    // If an intersection between this transaction and another in the arc exists,
	    // then there is a conflict. We extend the arc transaction vector to include
	    // the conflicting transaction. 
	    if !self.arcs[i].0.is_disjoint(&tx.inputs) {
		self.arcs[i].1.extend(vec![tx.clone()]);
		// If the inputs are an exact match then we do not need to add a separate
		// entry for this set of inputs.
		if self.arcs[i].0.inputs == tx.clone().inputs.inputs {
		    exact_match = true;
		}
	    }
	}
	// If the inputs were not an exact match then we must add this entry -- note that
	// the preferred transaction here is set to the `tx` supplied.
	if !exact_match {
	    self.arcs.push((tx.inputs.clone(), vec![tx.clone()], tx.clone()))
	}
    }

    pub fn insert_new(&mut self, tx: Tx) -> Result<()> {
	for i in 0..self.arcs.len() {
	    if !self.arcs[i].0.is_disjoint(&tx.inputs) {
		return Err(Error::DuplicateInputs);
	    }
	}
	self.arcs.push((tx.inputs.clone(), vec![tx.clone()], tx.clone()));
	Ok(())
    }
}

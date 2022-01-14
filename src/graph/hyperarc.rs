use super::{Error, Result};

use crate::chain::alpha::tx::{Input, Inputs, Outputs, Tx};
use crate::sleet::conflict_set::ConflictSet;

use std::collections::HashSet;

use std::cmp::Ord;
use std::hash::Hash;

#[derive(Debug)]
pub struct Hyperarc {
    arcs: Vec<(Inputs<Input>, ConflictSet<Tx>)>,
}

impl Hyperarc {
    pub fn new() -> Self {
        Hyperarc { arcs: vec![] }
    }

    pub fn get(&self, inputs: &Inputs<Input>) -> Option<ConflictSet<Tx>> {
        // A vector containing the transactions which conflict on the supplied inputs
        let mut conflicts = HashSet::new();
        // The transitive properties of the conflict set. These properties are inherited from the
        // first conflict set found.
        let mut pref = None;
        let mut last = None;
        let mut cnt = 0u8;
        for i in 0..self.arcs.len() {
            // If the inputs intersect then we have a conflict
            if !self.arcs[i].0.is_disjoint(inputs) {
                // Checks the original inputs of the transaction for intersection between itself and
                // the supplied inputs. This is so that we exclude non-conflicting transactions from
                // the set.
                for tx in self.arcs[i].1.conflicts.iter() {
                    if !tx.inputs.is_disjoint(inputs) {
                        conflicts.insert(tx.clone());
                        // If the preference has not yet been set, then we set it to the preference
                        // of the first intersecting conflict set. This assumes that the arcs are
                        // sorted by order of insertion.
                        if pref.is_none() {
                            pref = Some(self.arcs[i].1.pref.clone());
                            last = Some(self.arcs[i].1.last.clone());
                            cnt = self.arcs[i].1.cnt.clone();
                        }
                    }
                }
            }
        }
        if conflicts.len() > 0 {
            let conflict_set =
                ConflictSet { conflicts: conflicts, pref: pref.unwrap(), last: last.unwrap(), cnt };
            Some(conflict_set)
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
                self.arcs[i].1.conflicts.extend(vec![tx.clone()]);
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
            self.arcs.push((tx.inputs.clone(), ConflictSet::new(tx.clone())));
        }
    }

    pub fn insert_new(&mut self, tx: Tx) -> Result<()> {
        for i in 0..self.arcs.len() {
            if !self.arcs[i].0.is_disjoint(&tx.inputs) {
                return Err(Error::DuplicateInputs);
            }
        }
        self.arcs.push((tx.inputs.clone(), ConflictSet::new(tx.clone())));
        Ok(())
    }
}

use super::{Result, Error};

use super::conflict_set::ConflictSet;
use super::spend_map::SpendMap;

use crate::chain::alpha::{Transaction, TxHash};

use std::collections::{HashSet, HashMap, hash_map::Entry};

pub struct ConflictMap {
    inner: HashMap<TxHash, ConflictSet<TxHash>>,
    spend_map: SpendMap,
}

impl ConflictMap {
    pub fn new() -> Self {
	ConflictMap {
	    inner: HashMap::default(),
	    spend_map: SpendMap::new(),
	}
    }

    /// Whether `Tx` is preferred within its conflicting set.
    pub fn is_preferred(&self, tx_hash: TxHash) -> Result<bool> {
	match self.inner.get(&tx_hash) {
	    Some(cs) =>
		Ok(cs.is_preferred(tx_hash)),
	    None =>
		Err(Error::InvalidTransactionHash(tx_hash.clone())),
	}
    }

    /// Fetch the preferred transaction.
    pub fn get_preferred(&self, tx_hash: TxHash) -> Result<TxHash> {
	match self.inner.get(&tx_hash) {
	    Some(cs) =>
		Ok(cs.pref),
	    None =>
		Err(Error::InvalidTransactionHash(tx_hash)),
	}
    }

    pub fn insert_tx(&mut self, tx: Transaction) -> Result<ConflictSet<TxHash>> {
	// Insert the transaction input output ids into the spend map.
	self.spend_map.insert_tx(tx.clone());

	// Fetch the currently conflicting tx hashes based on the spend map.
	let conflicting_txs = self.spend_map.conflicting_txs(tx.clone());

	// For each conflict, fetch the conflict set of the transaction being referenced
	// except for the self tx hash, which has not yet been defined.
	let self_tx_hash = tx.hash();
	for tx_hash in conflicting_txs.iter() {
	    // Skip the self tx_hash
	    if tx_hash.clone() == self_tx_hash.clone() {
		continue;
	    }
	    match self.inner.entry(tx_hash.clone()) {
		Entry::Occupied(mut o) => {
		    // If there is already a conflict set for this entry then add this
		    // tx_hash as a conflict.
		    let cs = o.get_mut();
		    cs.insert(self_tx_hash.clone());
		},
		Entry::Vacant(mut v) => {
		    // Otherwise this must be an error: All prior transactions should have
		    // at minimum a singleton conflict set.
		    return Err(Error::InvalidConflictSet);
		},
	    }
	}

	// The conflict set for this transaction is the largest set of existing conflicts.
	let mut largest_set = HashSet::new();
	for tx_hash in conflicting_txs.iter().cloned() {
	    largest_set.insert(tx_hash);
	}

	// Check whether the equivalence relation holds between the largest set and a pre
	// existing conflict set.
	let mut equivalence = None;
	for (tx_hash, conflict_set) in self.inner.clone() {
	    if conflict_set.is_equivalent(largest_set.clone()) {
		equivalence = Some(conflict_set.clone());
		break;
	    }
	}

	// If there is an equivalence relation between the largest existing conflicting set
	// for this transaction and another conflict set, then the relation becomes
	// transitive and a new conflict set is created for the transaction which is
	// derived from the largest set.
	match equivalence {
	    Some(equivalent_cs) => {
		self.inner.insert(self_tx_hash.clone(), equivalent_cs.clone());
		Ok(equivalent_cs)
	    },
	    None => {
		// Otherwise if the largest set has no equivalence, then a new conflict
		// set is created with the new transaction as preference.
		let mut cs = ConflictSet::new(self_tx_hash.clone());
		cs.set_conflicts(largest_set.clone());
		self.inner.insert(self_tx_hash.clone(), cs.clone());
		Ok(cs)
	    },
	}
    }

    pub fn update_conflict_set(&mut self, tx: Transaction, d1: u8, d2: u8) -> Result<()> {
	match self.inner.entry(tx.hash()) {
	    Entry::Occupied(mut o) => {
		let cs = o.get_mut();
		if d1 > d2 {
		    cs.pref = tx.hash();
		}
		if tx.hash() != cs.last {
		    cs.last = tx.hash();
		} else {
		    cs.cnt += 1;
		}
		Ok(())
	    },
	    Entry::Vacant(_) =>
		Err(Error::InvalidTransaction(tx.clone())),
	}
    }
}

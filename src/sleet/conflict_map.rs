use super::{Result, Error};
use super::conflict_set::ConflictSet;
use super::tx::{Tx, TxHash};

use std::collections::{HashMap, hash_map::Entry};

/// The conflict map stores transaction hashes with possible conflicts.
pub struct ConflictMap {
    inner: HashMap<TxHash, ConflictSet<TxHash>>,
}

impl ConflictMap {
    pub fn new() -> Self {
	ConflictMap { inner: HashMap::default() }
    }

    /// Whether a node is preferred within its conflict set.
    pub fn is_preferred(&self, t: TxHash) -> Result<bool> {
	match self.inner.get(&t) {
	    Some(cs) =>
		Ok(cs.is_preferred(t)),
	    None =>
		Err(Error::UndefinedNode(t)),
	}
    }

    /// Inserts a new node within some existing conflict set, or creates a singleton set.
    pub fn insert(&mut self, t: TxHash) {
	match self.inner.entry(t.clone()) {
	    Entry::Occupied(mut o) => {
		let cs = o.get_mut();
		let _ = cs.insert(t);
	    },
	    Entry::Vacant(mut v) => {
		let cs = ConflictSet::new(t);
		let _ = v.insert(cs);
	    },
	}
    }

    pub fn update_ancestor(&mut self, t: TxHash, d1: u8, d2: u8) -> Result<()> {
	match self.inner.entry(t.clone()) {
	    Entry::Occupied(mut o) => {
		let cs = o.get_mut();
		if d1 > d2 {
		    cs.pref = t;
		}
		if t != cs.last {
		    cs.last = t;
		} else {
		    cs.cnt += 1;
		}
		Ok(())
	    },
	    Entry::Vacant(_) =>
		Err(Error::InvalidAncestor),
	}
    }
}

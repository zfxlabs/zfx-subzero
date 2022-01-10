use crate::chain::alpha::tx::UTXOId;
use crate::chain::alpha::{Transaction, TxHash};

use std::collections::{hash_map::Entry, HashMap, HashSet};

pub struct SpendMap {
    inner: HashMap<UTXOId, HashSet<TxHash>>,
}

impl SpendMap {
    /// Creates a new spend map containing outputs that have been referenced in potentially
    /// conflicting transactions.
    pub fn new() -> Self {
        SpendMap { inner: HashMap::default() }
    }

    /// Inserts the output ids of all the inputs referenced in the supplied `Tx`.
    pub fn insert_tx(&mut self, tx: Transaction) {
        for input in tx.inputs().iter() {
            match self.inner.entry(input.utxo_id()) {
                Entry::Occupied(mut o) => {
                    // The output id already exists, thus insert the tx id of the supplied
                    // conflicting transaction.
                    let tx_hashes = o.get_mut();
                    tx_hashes.insert(tx.hash());
                }
                Entry::Vacant(mut v) => {
                    // This output id has no existing conflicts, thus insert a singleton.
                    let mut hs = HashSet::new();
                    hs.insert(tx.hash());
                    v.insert(hs);
                }
            }
        }
    }

    /// Returns the hashes of all transactions which conflict with the spent outputs of the
    /// supplied `Tx`.
    pub fn conflicting_txs(&self, tx: Transaction) -> HashSet<TxHash> {
        let mut hs: HashSet<TxHash> = HashSet::new();
        for input in tx.inputs().iter() {
            match self.inner.get(&input.utxo_id()) {
                Some(tx_hashes) => {
                    for tx_hash in tx_hashes.iter().cloned() {
                        hs.insert(tx_hash);
                    }
                }
                None => (),
            }
        }
        hs
    }
}

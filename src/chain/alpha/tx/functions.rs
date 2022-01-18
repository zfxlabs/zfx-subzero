use crate::{Error, Result};

use super::types::*;
use super::{Transaction, TxHash};

use zerocopy::AsBytes;

/// Have we seen this transaction a priori.
pub fn is_known_tx(db: &sled::Db, tx_hash: TxHash) -> Result<bool> {
    let key = Key::new(tx_hash);
    match db.contains_key(key.as_bytes()) {
        Ok(r) => Ok(r),
        Err(err) => Err(Error::Sled(err)),
    }
}

/// Inserts a new transaction.
pub fn insert_tx(db: &sled::Db, tx: Transaction) -> Result<Option<sled::IVec>> {
    let h = tx.hash();
    let encoded = bincode::serialize(&tx).unwrap();
    let key = Key::new(h);
    match db.insert(key.as_bytes(), encoded) {
        Ok(v) => Ok(v),
        Err(err) => Err(Error::Sled(err)),
    }
}

// Get a transaction by its hash
pub fn get_tx(db: &sled::Db, tx_hash: &TxHash) -> Result<Option<Transaction>> {
    let key = Key::new(tx_hash.clone());
    match db.get(key.as_bytes()) {
        Ok(Some(t)) => Ok(Some(bincode::deserialize(t.as_bytes()).unwrap())),
        Ok(None) => Ok(None),
        Err(e) => Err(Error::Sled(e)),
    }
}

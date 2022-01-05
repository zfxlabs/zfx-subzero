use crate::{Result, Error};
use super::tx::{Tx, TxHash};
use super::types::*;

/// Have we seen this transaction a priori.
pub fn known_tx(db: sled::Db, tx_hash: TxHash) {
    let key = Key::new(tx_hash);
    match db.contains_key(key.as_bytes()) {
	Ok(r) =>
	    Ok(r),
	Err(err) =>
	    Err(Error::Sled(err)),
    }
}

/// Inserts a new transaction.
pub fn insert_conflict_set(db: sled::Db, tx: Tx) -> Result<Option<sled::IVec>> {
    let h = tx.hash();
    let encoded = bincode::serialize(&tx).unwrap();
    let key = Key::new(h);
    match db.insert(key.as_bytes(), encoded) {
	Ok(v) =>
	    Ok(v),
	Err(err) =>
	    Err(Error::Sled(err)),
    }
}

// /// Iterates through all known transactions checking whether they have conflicting inputs
// /// and ultimately returning the conflicting sets.
// pub fn get_conflict_sets(db: sled::Db, tx: Tx) {}

// // Transitively apply a preference to the relevant conflicting sets.
// pub fn apply_conflicting_preference() {}

// // Transitively add a new transaction to conflicting sets.
// pub fn apply_conflicting_transaction() {}

/// Inserts a new transaction.
pub fn insert_tx(db: sled::Db, tx: Tx) -> Result<Option<sled::IVec>> {
    let h = tx.hash();
    let encoded = bincode::serialize(&tx).unwrap();
    let key = Key::new(h);
    match db.insert(key.as_bytes(), encoded) {
	Ok(v) =>
	    Ok(v),
	Err(err) =>
	    Err(Error::Sled(err)),
    }
}

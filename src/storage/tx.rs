use super::{Error, Result};

use crate::alpha::types::TxHash;
use crate::sleet::tx::{Tx, TxStatus};

use zerocopy::{AsBytes, FromBytes, Unaligned};

#[derive(Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C)]
pub struct Key {
    hash: [u8; 32],
}

impl Key {
    pub fn new(h: [u8; 32]) -> Key {
        Key { hash: h }
    }
}

/// Whether this tx exists in storage.
pub fn is_known_tx(db: &sled::Db, tx_hash: TxHash) -> Result<bool> {
    let key = Key::new(tx_hash);
    match db.contains_key(key.as_bytes()) {
        Ok(r) => Ok(r),
        Err(err) => Err(Error::Sled(err)),
    }
}

/// Inserts a new tx into storage.
pub fn insert_tx(db: &sled::Db, tx: Tx) -> Result<Option<sled::IVec>> {
    let h = tx.hash();
    let encoded = bincode::serialize(&tx)?;
    let key = Key::new(h);
    match db.insert(key.as_bytes(), encoded) {
        Ok(v) => Ok(v),
        Err(err) => Err(Error::Sled(err)),
    }
}

/// Fetches a transaction.
pub fn get_tx(db: &sled::Db, tx_hash: TxHash) -> Result<(TxHash, Tx)> {
    let key = Key::new(tx_hash);
    match db.get(key.as_bytes()) {
        Ok(Some(v)) => {
            let tx: Tx = bincode::deserialize(v.as_bytes())?;
            Ok((key.hash.clone(), tx))
        }
        Ok(None) => Err(Error::InvalidTx),
        Err(err) => Err(Error::Sled(err)),
    }
}

/// Checks if we have the transaction accepted in the database
pub fn is_accepted_tx(db: &sled::Db, tx_hash: &TxHash) -> Result<bool> {
    let key = Key::new(*tx_hash);
    match db.get(key.as_bytes()) {
        Ok(Some(v)) => {
            let tx: Tx = bincode::deserialize(v.as_bytes())?;
            Ok(tx.status == TxStatus::Accepted)
        }
        Ok(None) => Err(Error::InvalidTx),
        Err(err) => Err(Error::Sled(err)),
    }
}

/// Checks if we have the transaction marked as 'removed' in the database
pub fn is_removed_tx(db: &sled::Db, tx_hash: &TxHash) -> Result<bool> {
    let key = Key::new(*tx_hash);
    match db.get(key.as_bytes()) {
        Ok(Some(v)) => {
            let tx: Tx = bincode::deserialize(v.as_bytes())?;
            Ok(tx.status == TxStatus::Removed)
        }
        Ok(None) => Err(Error::InvalidTx),
        Err(err) => Err(Error::Sled(err)),
    }
}

/// Fetch and update a transaction in the DB. Returns the new value.
pub fn update_and_fetch<F>(db: &sled::Db, tx_hash: &TxHash, mut f: F) -> Result<Tx>
where
    F: FnMut(Option<Tx>) -> Option<Tx>,
{
    let key = Key::new(tx_hash.clone());
    let updated = db.update_and_fetch(key.as_bytes(), |maybe_tx| {
        let maybe_tx = if let Some(tx) = maybe_tx {
            Some(bincode::deserialize(tx.as_bytes()).ok()?)
        } else {
            None
        };

        let result = f(maybe_tx);
        if let Some(ref tx) = result {
            Some(bincode::serialize(tx).ok()?)
        } else {
            None
        }
    });
    match updated {
        Ok(Some(v)) => {
            let tx: Tx = bincode::deserialize(v.as_bytes())?;
            Ok(tx)
        }
        Ok(None) => Err(Error::InvalidTx),
        Err(err) => Err(Error::Sled(err)),
    }
}

pub fn set_status(db: &sled::Db, tx_hash: &TxHash, status: TxStatus) -> Result<()> {
    let result = update_and_fetch(db, tx_hash, |tx| {
        if let Some(mut tx) = tx {
            tx.status = status.clone();
            Some(tx)
        } else {
            None
        }
    });
    match result {
        Ok(_tx) => Ok(()),
        Err(error) => Err(error),
    }
}

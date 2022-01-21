use super::{Error, Result};

use crate::alpha::types::TxHash;
use crate::sleet::tx::Tx;

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

/// Fetches the genesis block (the first block in the database).
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

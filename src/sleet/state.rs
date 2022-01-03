use super::{Result, Error};
use super::tx::{Tx, TxHash};

use byteorder::BigEndian;
use zerocopy::{
    byteorder::U64, AsBytes, FromBytes, Unaligned,
};

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

pub fn exists_tx(db: sled::Db, tx: &Tx) -> Result<bool> {
    let key = Key::new(tx.hash());
    match db.contains_key(key.as_bytes()) {
	Ok(r) =>
	    Ok(r),
	Err(err) =>
	    Err(Error::Sled(err)),
    }
}

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


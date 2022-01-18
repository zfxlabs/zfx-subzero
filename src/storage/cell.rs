use super::{Error, Result};

use crate::cell::types::CellHash;
use crate::cell::Cell;

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

/// Whether this cell exists in storage.
pub fn is_known_cell(db: &sled::Db, cell_hash: CellHash) -> Result<bool> {
    let key = Key::new(cell_hash);
    match db.contains_key(key.as_bytes()) {
        Ok(r) => Ok(r),
        Err(err) => Err(Error::Sled(err)),
    }
}

/// Inserts a new cell into storage.
pub fn insert_cell(db: &sled::Db, cell: Cell) -> Result<Option<sled::IVec>> {
    let h = cell.hash();
    let encoded = bincode::serialize(&cell)?;
    let key = Key::new(h);
    match db.insert(key.as_bytes(), encoded) {
        Ok(v) => Ok(v),
        Err(err) => Err(Error::Sled(err)),
    }
}

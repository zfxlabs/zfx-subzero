use super::{Error, Result};

use crate::alpha::types::BlockHash;
use crate::hail::block::HailBlock;

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

/// Whether this block exists in storage.
pub fn is_known_block(db: &sled::Db, block_hash: BlockHash) -> Result<bool> {
    let key = Key::new(block_hash);
    match db.contains_key(key.as_bytes()) {
        Ok(r) => Ok(r),
        Err(err) => Err(Error::Sled(err)),
    }
}

/// Inserts a new block into storage.
pub fn insert_block(db: &sled::Db, block: HailBlock) -> Result<Option<sled::IVec>> {
    let h = block.hash()?;
    let encoded = bincode::serialize(&block)?;
    let key = Key::new(h);
    match db.insert(key.as_bytes(), encoded) {
        Ok(v) => Ok(v),
        Err(err) => Err(Error::Sled(err)),
    }
}

/// Fetches a hail block.
pub fn get_block(db: &sled::Db, block_hash: BlockHash) -> Result<(BlockHash, HailBlock)> {
    let key = Key::new(block_hash);
    match db.get(key.as_bytes()) {
        Ok(Some(v)) => {
            let block: HailBlock = bincode::deserialize(v.as_bytes())?;
            Ok((key.hash.clone(), block))
        }
        Ok(None) => Err(Error::InvalidHailBlock),
        Err(err) => Err(Error::Sled(err)),
    }
}

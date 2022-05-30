use super::{Error, Result};
use crate::alpha::block::Block;
use crate::alpha::types::{BlockHash, BlockHeight};

use byteorder::BigEndian;
use zerocopy::{byteorder::U64, AsBytes, FromBytes, Unaligned};

#[derive(Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C)]
pub struct Key {
    pub height: U64<BigEndian>,
    pub hash: [u8; 32],
}

impl Key {
    pub fn new(height: u64, hash: BlockHash) -> Key {
        Key { height: U64::new(height), hash }
    }
}

#[derive(Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C)]
pub struct KeyPrefix {
    pub height: U64<BigEndian>,
}

impl KeyPrefix {
    pub fn new(key: &Key) -> KeyPrefix {
        KeyPrefix { height: key.height.clone() }
    }
}

/// Checks if the genesis block exists (the first block in the database).
pub fn exists_genesis(db: &sled::Db) -> bool {
    if let Ok(Some(_)) = db.first() {
        true
    } else {
        false
    }
}

/// Fetches the genesis block (the first block in the database).
pub fn get_genesis(db: &sled::Db) -> Result<(BlockHash, Block)> {
    match db.first() {
        Ok(Some((k, v))) => {
            let key: Key = Key::read_from(k.as_bytes()).unwrap();
            let block: Block = bincode::deserialize(v.as_bytes())?;
            Ok((key.hash.clone(), block))
        }
        Ok(None) => Err(Error::InvalidGenesis),
        Err(err) => Err(Error::Sled(err)),
    }
}

/// Inserts the genesis block into the database, returning its hash.
pub fn accept_genesis(db: &sled::Db, genesis: Block) -> Result<BlockHash> {
    let encoded = bincode::serialize(&genesis)?;
    let key = Key::new(genesis.height(), genesis.hash()?);
    let _ = db.insert(key.as_bytes(), encoded.clone())?;
    let h = genesis.hash()?;
    Ok(h)
}

/// Accepts a next block, ensuring that the previous block height = `height - 1`.
pub fn accept_next_block(db: sled::Db, block: Block) -> Result<BlockHash> {
    match db.last() {
        Ok(Some((k, _v))) => {
            let key: Key = Key::read_from(k.as_bytes()).ok_or(Error::InvalidLast)?;
            // check that height(block) = predecessor.height + 1
            if block.height() != u64::from(key.height) - 1u64 {
                return Err(Error::InvalidHeight);
            }
            // check that block.predecessor = hash(predecessor_block)
            if block.predecessor() != Some(key.hash) {
                return Err(Error::InvalidPredecessor);
            }
            // insert accepted block
            let encoded = bincode::serialize(&block)?;
            let hash = block.hash()?;
            let key = Key::new(block.height(), hash.clone());
            let _ = db.insert(key.as_bytes(), encoded.clone())?;
            Ok(hash)
        }
        Ok(None) => Err(Error::UndefinedGenesis),
        Err(err) => Err(Error::Sled(err)),
    }
}

/// Checks whether the block hash at a given height is exists.
pub fn is_known_block(db: &sled::Db, h: BlockHeight, block_hash: BlockHash) -> Result<bool> {
    let key = Key::new(h, block_hash);
    match db.contains_key(key.as_bytes()) {
        Ok(r) => Ok(r),
        Err(err) => Err(Error::Sled(err)),
    }
}

/// Inserts a new block into the database.
pub fn insert_block(db: &sled::Db, block: Block) -> Result<Option<sled::IVec>> {
    let encoded = bincode::serialize(&block)?;
    let key = Key::new(block.height(), block.hash()?);
    match db.insert(key.as_bytes(), encoded) {
        Ok(v) => Ok(v),
        Err(err) => Err(Error::Sled(err)),
    }
}

/// Gets the last stored blocks hash.
pub fn get_last_accepted_hash(db: &sled::Db) -> Result<BlockHash> {
    match db.last() {
        Ok(Some((k, _))) => {
            let key: Key = Key::read_from(k.as_bytes()).unwrap();
            Ok(key.hash.clone())
        }
        Ok(None) => Err(Error::InvalidLast),
        Err(err) => Err(Error::Sled(err)),
    }
}

/// Gets the last accepted block and its hash.
pub fn get_last_accepted(db: &sled::Db) -> Result<(BlockHash, Block)> {
    match db.last() {
        Ok(Some((k, v))) => {
            let key: Key = Key::read_from(k.as_bytes()).unwrap();
            Ok((key.hash.clone(), bincode::deserialize(v.as_bytes())?))
        }
        Ok(None) => Err(Error::InvalidLast),
        Err(err) => Err(Error::Sled(err)),
    }
}

/// Gets all blocks within a specific range of heights / hashes.
pub fn get_blocks_in_range(
    db: sled::Db,
    start_height: u64,
    start_hash: BlockHash,
    end_height: u64,
    end_hash: BlockHash,
) -> Result<Vec<Block>> {
    let mut blocks = vec![];
    let start = Key::new(start_height, start_hash);
    let end = Key::new(end_height, end_hash);
    for kv in db.range(start.as_bytes()..end.as_bytes()).rev() {
        match kv {
            Ok((_k, v)) => {
                let block = bincode::deserialize(v.as_bytes())?;
                blocks.push(block);
            }
            Err(err) => return Err(Error::Sled(err)),
        }
    }
    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alpha::block::{build_genesis, Block};

    #[actix_rt::test]
    async fn test_block_height_prefix() {
        // Create a test db
        let db = sled::Config::new().temporary(true).open().unwrap();

        let vout = [0u8; 32];

        // Construct test blocks
        let block0 = build_genesis().unwrap();
        let hash0 = block0.hash().unwrap();
        let encoded0 = bincode::serialize(&block0).unwrap();

        let block1 = Block::new(hash0.clone(), 1u64, vout, vec![]);
        let hash1 = block1.hash().unwrap();
        let encoded1 = bincode::serialize(&block1).unwrap();

        let key0 = Key::new(block0.height, hash0);
        let _ = db.insert(key0.as_bytes(), encoded0.clone()).unwrap();
        assert_eq!(db.get(key0.as_bytes()).unwrap().unwrap(), (encoded0.clone()));

        let prefix = KeyPrefix::new(&key0);
        let mut r0 = db.scan_prefix(prefix.as_bytes());
        assert_eq!(
            r0.next().unwrap().unwrap(),
            (sled::IVec::from(key0.as_bytes()), sled::IVec::from(encoded0.clone()),)
        );
        assert_eq!(r0.next(), None);

        let key1 = Key::new(block1.height, hash1);
        let _ = db.insert(key1.as_bytes(), encoded1.clone()).unwrap();
        assert_eq!(db.get(key1.as_bytes()).unwrap().unwrap(), (encoded1.clone()));

        let prefix = KeyPrefix::new(&key0);
        let mut r1 = db.scan_prefix(prefix.as_bytes());
        assert_eq!(
            r1.next().unwrap().unwrap(),
            (sled::IVec::from(key0.as_bytes()), sled::IVec::from(encoded0))
        );
        assert_eq!(r1.next(), None);
    }

    #[actix_rt::test]
    async fn test_block_height_ordering() {
        // Create a test db
        let db = sled::Config::new().temporary(true).open().unwrap();

        let vout = [0u8; 32];

        // Construct and insert test blocks
        let block0 = build_genesis().unwrap();
        let hash0 = block0.hash().unwrap();
        let encoded0 = bincode::serialize(&block0).unwrap();
        let block1 = Block::new(hash0.clone(), 1u64, vout, vec![]);
        let hash1 = block1.hash().unwrap();
        let encoded1 = bincode::serialize(&block1).unwrap();
        let block2 = Block::new(hash1.clone(), 2u64, vout, vec![]);
        let hash2 = block2.hash().unwrap();
        let encoded2 = bincode::serialize(&block2).unwrap();

        let key0 = Key::new(block0.height, hash0);
        let _ = db.insert(key0.as_bytes(), encoded0.clone()).unwrap();
        assert_eq!(db.get(key0.as_bytes()).unwrap().unwrap(), (encoded0.clone()));

        let key1 = Key::new(block1.height, hash1);
        let _ = db.insert(key1.as_bytes(), encoded1.clone()).unwrap();
        assert_eq!(db.get(key1.as_bytes()).unwrap().unwrap(), (encoded1.clone()));

        let key2 = Key::new(block2.height, hash2);
        let _ = db.insert(key2.as_bytes(), encoded2.clone()).unwrap();
        assert_eq!(db.get(key2.as_bytes()).unwrap().unwrap(), (encoded2.clone()));

        // Check first
        assert_eq!(
            db.first().unwrap().unwrap(),
            (sled::IVec::from(key0.as_bytes()), sled::IVec::from(encoded0.clone()),)
        );

        // Check last
        assert_eq!(
            db.last().unwrap().unwrap(),
            (sled::IVec::from(key2.as_bytes()), sled::IVec::from(encoded2.clone()),)
        );

        // Check standard ordering based on the key prefix
        let start = KeyPrefix::new(&key0);
        let end = KeyPrefix { height: U64::new(3u64) };
        let mut r0 = db.range(start.as_bytes()..end.as_bytes());
        assert_eq!(
            r0.next().unwrap().unwrap(),
            (sled::IVec::from(key0.as_bytes()), sled::IVec::from(encoded0.clone()),)
        );
        assert_eq!(
            r0.next().unwrap().unwrap(),
            (sled::IVec::from(key1.as_bytes()), sled::IVec::from(encoded1.clone()),)
        );
        assert_eq!(
            r0.next().unwrap().unwrap(),
            (sled::IVec::from(key2.as_bytes()), sled::IVec::from(encoded2.clone()),)
        );
        assert_eq!(r0.next(), None);

        // Check reverse traversal ordering
        let mut r1 = db.range(start.as_bytes()..end.as_bytes()).rev();
        assert_eq!(
            r1.next().unwrap().unwrap(),
            (sled::IVec::from(key2.as_bytes()), sled::IVec::from(encoded2.clone()),)
        );
        assert_eq!(
            r1.next().unwrap().unwrap(),
            (sled::IVec::from(key1.as_bytes()), sled::IVec::from(encoded1.clone()),)
        );
        assert_eq!(
            r1.next().unwrap().unwrap(),
            (sled::IVec::from(key0.as_bytes()), sled::IVec::from(encoded0.clone()),)
        );
        assert_eq!(r1.next(), None);
    }
}

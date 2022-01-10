use crate::{Error, Result};

use super::block::*;
use super::types::*;

use zerocopy::{AsBytes, FromBytes};

//-- Fetching / inserting `genesis`

pub fn exists_first(db: &sled::Db) -> bool {
    if let Ok(Some(_)) = db.first() {
        true
    } else {
        false
    }
}

pub fn get_genesis(db: &sled::Db) -> Result<(BlockHash, Block)> {
    match db.first() {
        Ok(Some((k, v))) => {
            let key: Key = Key::read_from(k.as_bytes()).unwrap();
            Ok((key.hash.clone(), bincode::deserialize(v.as_bytes()).unwrap()))
        }
        Ok(None) => Err(Error::InvalidGenesis),
        Err(err) => Err(Error::Sled(err)),
    }
}

pub fn accept_genesis(db: &sled::Db, genesis: Block) -> BlockHash {
    let encoded = bincode::serialize(&genesis).unwrap();
    let key = Key::new(genesis.height, genesis.hash());
    let _ = db.insert(key.as_bytes(), encoded.clone()).unwrap();
    genesis.hash()
}

pub fn accept(db: sled::Db, block: Block) -> Result<BlockHash> {
    match db.last() {
        Ok(Some((k, v))) => {
            let key: Key = Key::read_from(k.as_bytes()).unwrap();
            // check that height(block) = predecessor.height + 1
            if block.height != u64::from(key.height) - 1u64 {
                return Err(Error::InvalidHeight);
            }
            // check that block.predecessor = hash(predecessor_block)
            if block.predecessor != Some(key.hash) {
                return Err(Error::InvalidPredecessor);
            }
            // insert accepted block
            let encoded = bincode::serialize(&block).unwrap();
            let hash = block.hash();
            let key = Key::new(block.height, hash.clone());
            let _ = db.insert(key.as_bytes(), encoded.clone()).unwrap();
            Ok(hash)
        }
        Ok(None) => Err(Error::GenesisUndefined),
        Err(err) => Err(Error::Sled(err)),
    }
}

//-- Fetching the last accepted blocks and their ancestors

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

pub fn get_last_accepted(db: &sled::Db) -> Result<(BlockHash, Block)> {
    match db.last() {
        Ok(Some((k, v))) => {
            let key: Key = Key::read_from(k.as_bytes()).unwrap();
            Ok((key.hash.clone(), bincode::deserialize(v.as_bytes()).unwrap()))
        }
        Ok(None) => Err(Error::InvalidLast),
        Err(err) => Err(Error::Sled(err)),
    }
}

pub fn get_ancestors(
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
            Ok((k, v)) => {
                let block = bincode::deserialize(v.as_bytes()).unwrap();
                blocks.push(block);
            }
            Err(err) => return Err(Error::Sled(err)),
        }
    }
    Ok(blocks)
}

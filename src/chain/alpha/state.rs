use zfx_id::Id;

use crate::colored::Colorize;

use crate::{Result, Error};
use super::block::{Block, BlockHash};

use byteorder::BigEndian;
use zerocopy::{
    byteorder::U64, AsBytes, FromBytes, Unaligned,
};

use tai64::Tai64;

use std::collections::HashSet;

#[derive(Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C)]
pub struct Key {
    height: U64<BigEndian>,
    hash: [u8; 32],
}

impl Key {
    pub fn new(height: u64, hash: BlockHash) -> Key {
	Key {
	    height: U64::new(height),
	    hash,
	}
    }
}

#[derive(Clone, FromBytes, AsBytes, Unaligned)]
#[repr(C)]
pub struct KeyPrefix {
    height: U64<BigEndian>,
}

impl KeyPrefix {
    pub fn new(key: &Key) -> KeyPrefix {
	KeyPrefix { height: key.height.clone() }
    }
}

//-- Fetching / inserting `genesis`

pub fn exists_first(db: &sled::Db) -> bool {
    if let Ok(Some(_)) = db.first() {
	true
    } else {
	false
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
	},
	Ok(None) =>
	    Err(Error::GenesisUndefined),
	Err(err) =>
	    Err(Error::Sled(err)),
    }
}

//-- Fetching the last accepted blocks and their ancestors

pub fn get_last_accepted_hash(db: &sled::Db) -> Result<BlockHash> {
    match db.last() {
	Ok(Some((k, _))) => {
	    let key: Key = Key::read_from(k.as_bytes()).unwrap();
	    Ok(key.hash.clone())
	}
	Ok(None) =>
	    Err(Error::InvalidLast),
	Err(err) =>
	    Err(Error::Sled(err)),
    }
}

pub fn get_last_accepted(db: &sled::Db) -> Result<(BlockHash, Block)> {
    match db.last() {
	Ok(Some((k, v))) => {
	    let key: Key = Key::read_from(k.as_bytes()).unwrap();
	    Ok((key.hash.clone(), bincode::deserialize(v.as_bytes()).unwrap()))
	},
	Ok(None) =>
	    Err(Error::InvalidLast),
	Err(err) =>
	    Err(Error::Sled(err)),
    }
}

pub fn get_ancestors(db: sled::Db, start_height: u64, start_hash: BlockHash, end_height: u64, end_hash: BlockHash) -> Result<Vec<Block>> {
    let mut blocks = vec![];
    let start = Key::new(start_height, start_hash);
    let end = Key::new(end_height, end_hash);
    for kv in db.range(start.as_bytes()..end.as_bytes()).rev() {
	match kv {
	    Ok((k, v)) => {
		let block = bincode::deserialize(v.as_bytes()).unwrap();
		blocks.push(block);
	    },
	    Err(err) =>
		return Err(Error::Sled(err)),
	}
    }
    Ok(blocks)
}

//-- State resulting from the application of blocks

pub type Weight = f64;

#[derive(Debug, Clone)]
pub struct State {
    pub height: u64,
    pub total_tokens: u64,
    pub validators: Vec<(Id, u64)>,
}

//-- Apply transactions to the current state

impl State {
    pub fn new() -> State {
	State { height: 0, total_tokens: 0, validators: vec![] }
    }

    pub fn apply(&mut self, block: Block) {
	self.height = block.height;
	
	for stake_tx in block.txs.iter() {
	    self.total_tokens += stake_tx.qty;
	}
	// TODO: For testing purposes we make every validator stake forever
	for stake_tx in block.txs.iter() {
	    // if stake_tx.start_time <= Tai64::now() && stake_tx.end_time >= Tai64::now() {
	    //let w: f64 = percent_of(stake_tx.qty, self.total_tokens);
	    self.validators.push((stake_tx.node_id.clone(), stake_tx.qty));
	    //}
	}
    }

    pub fn format(&self) -> String {
	let total_tokens = format!("Σ = {:?}", self.total_tokens).cyan();
	let mut s: String = format!("{}\n", total_tokens);
	for (id, w) in self.validators.clone() {
	    let id_s = format!("{:?}", id).yellow();
	    let w_s = format!("{:?}", w).magenta();
	    s = format!("{} ν = {} {} | {} {}\n", s, "⦑".cyan(), id_s, w_s, "⦒".cyan());
	}
	s
    }
}

//-- State persistence functions

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::alpha::block::{Block, genesis};

    // use tracing_subscriber;
    // use tracing::info;

    #[actix_rt::test]
    async fn test_height_prefix() {
	// Create a test db
        let db = sled::Config::new().temporary(true).open().unwrap();

	let vout = [0u8; 32];

	// Construct test blocks
	let block0 = genesis();
	let hash0 = block0.hash();
	let encoded0 = bincode::serialize(&block0).unwrap();

	let block1 = Block::new(hash0.clone(), 1u64, vout, vec![]);
	let hash1 = block1.hash();
	let encoded1 = bincode::serialize(&block1).unwrap();

	let key0 = Key::new(block0.height, hash0);
	let _ = db.insert(key0.as_bytes(), encoded0.clone()).unwrap();
	assert_eq!(db.get(key0.as_bytes()).unwrap().unwrap(), (encoded0.clone()));

	let prefix = KeyPrefix::new(&key0);
	let mut r0 = db.scan_prefix(prefix.as_bytes());
	assert_eq!(r0.next().unwrap().unwrap(), (
	    sled::IVec::from(key0.as_bytes()),
	    sled::IVec::from(encoded0.clone()),
	));
	assert_eq!(r0.next(), None);

	let key1 = Key::new(block1.height, hash1);
	let _ = db.insert(key1.as_bytes(), encoded1.clone()).unwrap();
	assert_eq!(db.get(key1.as_bytes()).unwrap().unwrap(), (encoded1.clone()));
	
	let prefix = KeyPrefix::new(&key0);
	let mut r1 = db.scan_prefix(prefix.as_bytes());
	assert_eq!(r1.next().unwrap().unwrap(), (
	    sled::IVec::from(key0.as_bytes()), sled::IVec::from(encoded0)
	));
	assert_eq!(r1.next(), None);
    }

    #[actix_rt::test]
    async fn test_height_ordering() {
	// Create a test db
        let db = sled::Config::new().temporary(true).open().unwrap();

	let vout = [0u8; 32];

	// Construct and insert test blocks
	let block0 = genesis();
	let hash0 = block0.hash();
	let encoded0 = bincode::serialize(&block0).unwrap();
	let block1 = Block::new(hash0.clone(), 1u64, vout, vec![]);
	let hash1 = block1.hash();
	let encoded1 = bincode::serialize(&block1).unwrap();
	let block2 = Block::new(hash1.clone(), 2u64, vout, vec![]);
	let hash2 = block2.hash();
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
	assert_eq!(db.first().unwrap().unwrap(), (
	    sled::IVec::from(key0.as_bytes()),
	    sled::IVec::from(encoded0.clone()),
	));
	
	// Check last
	assert_eq!(db.last().unwrap().unwrap(), (
	    sled::IVec::from(key2.as_bytes()),
	    sled::IVec::from(encoded2.clone()),
	));

	// Check standard ordering based on the key prefix
	let start = KeyPrefix::new(&key0);
	let end = KeyPrefix { height: U64::new(3u64) };
	let mut r0 = db.range(start.as_bytes()..end.as_bytes());
	assert_eq!(r0.next().unwrap().unwrap(), (
	    sled::IVec::from(key0.as_bytes()),
	    sled::IVec::from(encoded0.clone()),
	));
	assert_eq!(r0.next().unwrap().unwrap(), (
	    sled::IVec::from(key1.as_bytes()),
	    sled::IVec::from(encoded1.clone()),
	));
	assert_eq!(r0.next().unwrap().unwrap(), (
	    sled::IVec::from(key2.as_bytes()),
	    sled::IVec::from(encoded2.clone()),
	));
	assert_eq!(r0.next(), None);

	// Check reverse traversal ordering
	let mut r1 = db.range(start.as_bytes()..end.as_bytes()).rev();
	assert_eq!(r1.next().unwrap().unwrap(), (
	    sled::IVec::from(key2.as_bytes()),
	    sled::IVec::from(encoded2.clone()),
	));
	assert_eq!(r1.next().unwrap().unwrap(), (
	    sled::IVec::from(key1.as_bytes()),
	    sled::IVec::from(encoded1.clone()),
	));
	assert_eq!(r1.next().unwrap().unwrap(), (
	    sled::IVec::from(key0.as_bytes()),
	    sled::IVec::from(encoded0.clone()),
	));
	assert_eq!(r1.next(), None);
    }
}

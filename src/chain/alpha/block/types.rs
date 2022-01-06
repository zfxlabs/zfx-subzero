use super::block::BlockHash;

use byteorder::BigEndian;
use zerocopy::{
    byteorder::U64, AsBytes, FromBytes, Unaligned,
};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::alpha::block::{Block, genesis};

    // use tracing_subscriber;
    // use tracing::info;

    #[actix_rt::test]
    async fn test_block_height_prefix() {
	// Create a test db
        let db = sled::Config::new().temporary(true).open().unwrap();

	let vout = [0u8; 32];

	// Construct test blocks
	let block0 = genesis(vec![]);
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
    async fn test_block_height_ordering() {
	// Create a test db
        let db = sled::Config::new().temporary(true).open().unwrap();

	let vout = [0u8; 32];

	// Construct and insert test blocks
	let block0 = genesis(vec![]);
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

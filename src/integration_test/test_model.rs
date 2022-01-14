use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use ed25519_dalek::Keypair;
use crate::chain::alpha::TxHash;

pub const KEYPAIR_NODE_0 : &str = "ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416";
pub const KEYPAIR_NODE_1 : &str = "5a353c630d3faf8e2d333a0983c1c71d5e9b6aed8f4959578fbeb3d3f3172886393b576de0ac1fe86a4dd416cf032543ac1bd066eb82585f779f6ce21237c0cd";
pub const KEYPAIR_NODE_2 : &str = "6f4b736b9a6894858a81696d9c96cbdacf3d49099d212213f5abce33da18716f067f8a2b9aeb602cd4163291ebbf39e0e024634f3be19bde4c490465d9095a6b";
pub const NODE_ADDRESS : &str = "127.0.0.1:123";

pub struct IntegrationTestContext {
    pub test_run_counter: u32,
    old_to_new_tx_hashes: HashMap<TxHash, TxHash>,
}

impl IntegrationTestContext {
    pub fn new() -> IntegrationTestContext {
        IntegrationTestContext { test_run_counter: 0, old_to_new_tx_hashes: HashMap::new() }
    }

    pub fn count_test_run(&mut self) {
        self.test_run_counter += 1;
    }

    /// Attach the latest hash of tx into the list to be able to identify the
    /// last spent tx, related to the root tx
    pub fn register_tx_hash(&mut self, old_tx_hash: TxHash, next_tx_hash: TxHash) {
        self.old_to_new_tx_hashes.insert(old_tx_hash, next_tx_hash);
    }

    /// Get the last spent tx of root tx
    pub fn get_last_tx_of(&self, tx_hash: &TxHash) -> TxHash {
        match self.old_to_new_tx_hashes.get(tx_hash) {
            Some(x) => {
                if x != tx_hash && self.old_to_new_tx_hashes.contains_key(x) {
                    self.get_last_tx_of(x)
                } else {
                    x.clone()
                }
            }
            _ => tx_hash.clone(),
        }
    }

    /// Takes the tx hashes and identifies which ones are last (can be spent)
    pub fn get_latest_txs_of(&self, tx_hashes: Vec<TxHash>) -> HashSet<TxHash> {
        tx_hashes.iter().map(|entry| self.get_last_tx_of(entry)).collect::<HashSet<TxHash>>()
    }
}

pub struct TestNodes {
    nodes: Vec<TestNode>,
}

impl TestNodes {
    pub fn new() -> Self {
        let mut nodes = vec![];
        nodes.push(TestNode::new(0, KEYPAIR_NODE_0));
        nodes.push(TestNode::new(1, KEYPAIR_NODE_1));
        nodes.push(TestNode::new(2, KEYPAIR_NODE_2));

        TestNodes { nodes }
    }

    pub fn get_node(&self, id: usize) -> Option<&TestNode> {
        return self.nodes.get(id);
    }
}

pub struct TestNode {
    pub keypair: Keypair,
    pub public_key: [u8; 32],
    pub address: SocketAddr,
}

impl TestNode {
    pub fn new(id: u32, keypair: &str) -> Self {
        let (kp, pkh) = TestNode::create_keys_of_node(keypair);
        let mut address = String::from(NODE_ADDRESS);
        address.push_str((id + 4).to_string().borrow());  // port of node 0 ends in 4, node 1 in 5, etc.

        TestNode {
            keypair: kp,
            public_key: pkh,
            address: address.parse().expect("failed to construct address"),
        }
    }

    fn create_keys_of_node(keypair: &str) -> (Keypair, [u8; 32]) {
        let keypair_bytes = hex::decode(keypair).unwrap();
        let keypair = Keypair::from_bytes(&keypair_bytes).unwrap();
        let encoded = bincode::serialize(&keypair.public).unwrap();
        let pkh = blake3::hash(&encoded).as_bytes().clone();
        (keypair, pkh)
    }
}

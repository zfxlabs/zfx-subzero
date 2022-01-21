use crate::cell::types::CellHash;
use crate::integration_test::test_actix_node::TestActixThread;
use crate::server::node::run;
use ed25519_dalek::Keypair;
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

pub const KEYPAIR_NODE_0 : &str = "ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416";
pub const KEYPAIR_NODE_1 : &str = "5a353c630d3faf8e2d333a0983c1c71d5e9b6aed8f4959578fbeb3d3f3172886393b576de0ac1fe86a4dd416cf032543ac1bd066eb82585f779f6ce21237c0cd";
pub const KEYPAIR_NODE_2 : &str = "6f4b736b9a6894858a81696d9c96cbdacf3d49099d212213f5abce33da18716f067f8a2b9aeb602cd4163291ebbf39e0e024634f3be19bde4c490465d9095a6b";
pub const NON_EXISTING_NODE : &str = "9f4b736b9a6894858a81696d9c96cbdacf3d49099d212213f5abce33da18716f067f8a2b9aeb602cd4163291ebbf39e0e024634f3be19bde4c490465d9095a6b";
pub const NODE_ADDRESS: &str = "127.0.0.1:123";

pub struct IntegrationTestContext {
    pub test_run_counter: u32,
    pub test_nodes: TestNodes,
    old_to_new_cell_hashes: HashMap<CellHash, CellHash>,
}

impl IntegrationTestContext {
    pub fn new() -> IntegrationTestContext {
        IntegrationTestContext {
            test_run_counter: 0,
            old_to_new_cell_hashes: HashMap::new(),
            test_nodes: TestNodes::new(),
        }
    }

    pub fn count_test_run(&mut self) {
        self.test_run_counter += 1;
    }

    /// Attach the latest hash of the cell into the list to be able to identify the
    /// last spent cell, related to the root cell
    pub fn register_cell_hash(&mut self, old_cell_hash: CellHash, next_cell_hash: CellHash) {
        self.old_to_new_cell_hashes.insert(old_cell_hash, next_cell_hash);
    }

    /// Get the last spent cell of the root cell
    pub fn get_last_cell_of(&self, cell_hash: &CellHash) -> CellHash {
        match self.old_to_new_cell_hashes.get(cell_hash) {
            Some(x) => {
                if x != cell_hash && self.old_to_new_cell_hashes.contains_key(x) {
                    self.get_last_cell_of(x)
                } else {
                    x.clone()
                }
            }
            _ => cell_hash.clone(),
        }
    }

    /// Takes the tx hashes and identifies which ones are last (can be spent)
    pub fn get_latest_cells_of(&self, cell_hashes: Vec<CellHash>) -> HashSet<CellHash> {
        cell_hashes.iter().map(|entry| self.get_last_cell_of(entry)).collect::<HashSet<CellHash>>()
    }
}

pub struct TestNodes {
    pub nodes: Vec<TestNode>,
}

impl TestNodes {
    pub fn new() -> Self {
        let mut nodes = vec![];
        nodes.push(TestNode::new(0, 1, KEYPAIR_NODE_0));
        nodes.push(TestNode::new(1, 0, KEYPAIR_NODE_1));
        nodes.push(TestNode::new(2, 1, KEYPAIR_NODE_2));

        TestNodes { nodes }
    }

    pub fn get_node(&self, id: usize) -> Option<&TestNode> {
        return self.nodes.get(id);
    }

    pub fn get_non_existing_node(&self) -> TestNode {
        return TestNode::new(9, 9, NON_EXISTING_NODE);
    }
}

pub struct TestNode {
    pub keypair: Keypair,
    pub public_key: [u8; 32],
    pub address: SocketAddr,
    pub keypair_as_str: String,
    pub address_as_str: String,
    pub bootstrap_address: String,
    pub state: ThreadNodeState,
}

pub enum ThreadNodeState {
    Stopped,
    Running(TestActixThread),
}

impl TestNode {
    pub fn new(id: u32, bootstrap_port: u32, keypair: &str) -> Self {
        let (kp, pkh) = TestNode::create_keys_of_node(keypair);
        let mut address = String::from(NODE_ADDRESS);
        let mut bootstrap_address = String::from(NODE_ADDRESS);
        address.push_str((id + 4).to_string().borrow()); // port of node 0 ends in 4, node 1 in 5, etc.
        bootstrap_address.push_str((bootstrap_port + 4).to_string().borrow()); // port of node 0 ends in 4, node 1 in 5, etc.

        TestNode {
            keypair: kp,
            public_key: pkh,
            address: address.parse().expect("failed to construct address"),
            keypair_as_str: String::from(keypair),
            address_as_str: address,
            bootstrap_address,
            state: ThreadNodeState::Stopped,
        }
    }

    pub fn start(&mut self) {
        let node_ip = self.address_as_str.clone();
        let bootstrap_ips = vec![self.bootstrap_address.clone()];
        let keypair = self.keypair_as_str.clone();

        let handler = TestActixThread::start(move || {
            run(node_ip, bootstrap_ips, Some(keypair)).unwrap();
        });

        self.state = ThreadNodeState::Running(handler);
    }

    pub fn kill(&mut self) {
        let state = std::mem::replace(&mut self.state, ThreadNodeState::Stopped);
        match state {
            ThreadNodeState::Stopped => panic!("Node is not running"),
            ThreadNodeState::Running(thread) => {
                thread.shutdown();
                self.state = ThreadNodeState::Stopped;
            }
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

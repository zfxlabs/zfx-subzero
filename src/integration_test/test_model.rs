use crate::cell::types::CellHash;
use crate::integration_test::test_functions::wait_until_nodes_start;
use crate::zfx_id::Id;
use crate::Error;
use ed25519_dalek::Keypair;
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::process::{Child, Command};
use std::time::Duration;
use std::{panic, thread};
use tracing::info;
use x509_parser::nom::AsBytes;

pub const KEYPAIR_NODE_0 : &str = "ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416";
pub const KEYPAIR_NODE_1 : &str = "5a353c630d3faf8e2d333a0983c1c71d5e9b6aed8f4959578fbeb3d3f3172886393b576de0ac1fe86a4dd416cf032543ac1bd066eb82585f779f6ce21237c0cd";
pub const KEYPAIR_NODE_2 : &str = "6f4b736b9a6894858a81696d9c96cbdacf3d49099d212213f5abce33da18716f067f8a2b9aeb602cd4163291ebbf39e0e024634f3be19bde4c490465d9095a6b";
pub const KEYPAIR_NODE_3 : &str = "3ae38eec96146c241f6cadf01995af14f027b23b8fecbc77dbc2e3ed5fec6fc3fb4fe5534f7affc9a8f1d99e290fdb91cc26777edd6fae480cad9f735d1b3680";
pub const KEYPAIR_NODE_4 : &str = "aae4e1343eb40e217a60fc61e22b86925686e664d7663c09d0042eb049600e187a2049a994e5b7a3e2baa9341c697029550ee0782d83ba31fe10fa0fefd6cc52";
pub const KEYPAIR_NODE_5 : &str = "8c739c713aeb69e21a37bc2aab2ab314d08627d5435754b0418a71529c3614bccdfa638fa8da6d06e98a374c1df48e3a3d2563a4c7d78d0e7589f6706a8ed0d8";
pub const KEYPAIR_NODE_6 : &str = "2d2ca57915c481e043744f265397fbec35c8c259c909c3ad365603b243db6de086eb292e6e4f65d14e7b35e3882af5f778924ff1fb95e815473d1eac583df1be";
pub const KEYPAIR_NODE_7 : &str = "845821ffb4a9c6f4a4dbdc63d3d6e2e3ac1ca3a78950d9f4240092ffff9a24f8ad42f4caf9cfa37b77cddd899c692e0fa5df3dbcd206685e367649ffe6834de4";
pub const KEYPAIR_NODE_8 : &str = "bffaff3355751ff98beea80841c90f79b6bf256268b83c13d2b68ab2ea168bf02eb8d83c4998f012b65d8b54d1ee3c2db2649495f92ed20881f34720c4a73755";
pub const NODE_ID_0: &str = "12My22AzQQosboCy6TCDFkTQwHTSuHhFN1VDcdDRPUe3H8j3DvY";
pub const NODE_ID_1: &str = "19Y53ymnBw4LWUpiAMUzPYmYqZmukRhNHm3VyAhzMqckRcuvkf";
pub const NODE_ID_2: &str = "1A2iUK1VQWMfvtmrBpXXkVJjM5eMWmTfMEcBx4TatSJeuoSH7n";
pub const NODE_ID_3: &str = "12StzamTJk2jBxbdqGmT6gLfpctv9f39CmBXTsm8sBG2n6AdPxx";
pub const NODE_ID_4: &str = "1tJB1qNY6R4nPGQN83hmX8bviD6dbEMXkGjfByrCVYZsNnrJSk";
pub const NODE_ID_5: &str = "12KyV3nz5wJhqFSfEFsKAhEqMGaPD88JeeS7LA4Qsjbyf2Yqp87";
pub const NON_EXISTING_NODE : &str = "9f4b736b9a6894858a81696d9c96cbdacf3d49099d212213f5abce33da18716f067f8a2b9aeb602cd4163291ebbf39e0e024634f3be19bde4c490465d9095a6b";
pub const NODE_ADDRESS: &str = "127.0.0.1:123";

pub struct IntegrationTestContext {
    pub test_run_counter: u32,
    old_to_new_cell_hashes: HashMap<CellHash, CellHash>,
}

impl IntegrationTestContext {
    pub fn new() -> IntegrationTestContext {
        IntegrationTestContext { test_run_counter: 0, old_to_new_cell_hashes: HashMap::new() }
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
        nodes.push(TestNode::new(0, 1, NODE_ID_1, KEYPAIR_NODE_0, NODE_ID_0));
        nodes.push(TestNode::new(1, 0, NODE_ID_0, KEYPAIR_NODE_1, NODE_ID_1));
        nodes.push(TestNode::new(2, 1, NODE_ID_1, KEYPAIR_NODE_2, NODE_ID_2));
        nodes.push(TestNode::new(3, 2, NODE_ID_2, KEYPAIR_NODE_3, NODE_ID_3));
        nodes.push(TestNode::new(4, 3, NODE_ID_3, KEYPAIR_NODE_4, NODE_ID_4));
        nodes.push(TestNode::new(5, 4, NODE_ID_4, KEYPAIR_NODE_5, NODE_ID_5));

        TestNodes { nodes }
    }

    pub fn get_running_nodes(&self) -> Vec<&TestNode> {
        return self
            .nodes
            .iter()
            .filter_map(|n| if let ProcessNodeState::Running(_) = n.state { Some(n) } else { None })
            .collect::<Vec<&TestNode>>();
    }

    pub fn get_node(&self, id: usize) -> Option<&TestNode> {
        return self.nodes.get(id);
    }

    pub fn get_non_existing_node(&self) -> TestNode {
        return TestNode::new(9, 9, NODE_ID_1, NON_EXISTING_NODE, NODE_ID_0);
    }

    pub fn kill_all(&mut self) {
        for node in &mut self.nodes {
            node.kill();
        }
    }

    pub fn kill_node(&mut self, id: usize) {
        self.nodes[id].kill();
    }

    pub fn start_node(&mut self, id: usize) {
        if let ProcessNodeState::Stopped = self.nodes[id].state {
            self.nodes[id].start();
        }
    }

    fn start_all(&mut self, node_ids: Vec<&str>) {
        for node in &mut self.nodes {
            if node_ids.contains(&node.id.as_str()) {
                node.start();
            }
        }
    }

    pub async fn start_minimal_and_wait(&mut self) -> std::result::Result<(), Error> {
        self.start_all(vec![NODE_ID_0, NODE_ID_1, NODE_ID_2]);
        wait_until_nodes_start(self).await
    }

    pub fn is_running(&self, node_id: usize) -> bool {
        match self.get_node(node_id) {
            Some(n) => match n.state {
                ProcessNodeState::Stopped => false,
                ProcessNodeState::Running(_) => true
            }
            None => false
        }
    }
}

impl Drop for TestNodes {
    fn drop(&mut self) {
        self.kill_all();
    }
}

pub struct TestNode {
    pub keypair: Keypair,
    pub public_key: [u8; 32],
    pub address: SocketAddr,
    pub keypair_as_str: String,
    pub address_as_str: String,
    pub bootstrap_address: String,
    pub state: ProcessNodeState,
    pub id: String,
}

pub enum ProcessNodeState {
    Stopped,
    Running(Child),
}

impl TestNode {
    pub fn new(
        id: u32,
        bootstrap_port: u32,
        bootstrap_node_id: &str,
        keypair: &str,
        node_id_str: &str,
    ) -> Self {
        let (kp, pkh) = TestNode::create_keys_of_node(keypair);
        let mut address = String::from(NODE_ADDRESS);
        let mut bootstrap_address =
            format!("{}@{}{}", bootstrap_node_id, NODE_ADDRESS, (bootstrap_port + 4).to_string());
        address.push_str((id + 4).to_string().borrow()); // port of node 0 ends in 4, node 1 in 5, etc.

        TestNode {
            id: String::from(node_id_str),
            keypair: kp,
            public_key: pkh,
            address: address.parse().expect("failed to construct address"),
            keypair_as_str: String::from(keypair),
            address_as_str: address,
            bootstrap_address,
            state: ProcessNodeState::Stopped,
        }
    }

    pub fn start(&mut self) {
        match self.state {
            ProcessNodeState::Stopped => {
                std::env::set_var("ADVERSARY_CONSENT", "1");
                let child =
                    self.get_start_node_command().spawn().expect("start node command failed");
                self.state = ProcessNodeState::Running(child);
            }
            ProcessNodeState::Running(_) => panic!("Node is already running"),
        }
    }

    pub fn kill(&mut self) {
        match self.state {
            ProcessNodeState::Running(ref mut child) => {
                info!("Shutting down the node {}", self.address_as_str);
                child.kill().expect("kill failed");
                thread::sleep(Duration::from_secs(1));
                self.state = ProcessNodeState::Stopped;
                info!("Node {} has been shut down", self.address_as_str);
            }
            ProcessNodeState::Stopped => info!("Node was already stopped"),
        }
    }

    fn create_keys_of_node(keypair: &str) -> (Keypair, [u8; 32]) {
        let keypair_bytes = hex::decode(keypair).unwrap();
        let keypair = Keypair::from_bytes(&keypair_bytes).unwrap();
        let encoded = bincode::serialize(&keypair.public).unwrap();
        let pkh = blake3::hash(&encoded).as_bytes().clone();
        (keypair, pkh)
    }

    /// Side effect: writes chain spec file
    pub fn get_start_node_command(&self) -> Command {
        let cargo_path =
            format!("{}/.cargo/bin/cargo", dirs::home_dir().unwrap().to_str().unwrap().to_string());
        let mut command = Command::new(cargo_path);
        command.args(&["run", "-p", "zfx-subzero"]);
        command.args(&["--bin", "node", "--", "-a"]);
        command.arg(&self.address_as_str);
        command.arg("-b");
        command.arg(&self.bootstrap_address);
        command.arg("--keypair");
        command.arg(&self.keypair_as_str);
        command.arg("--id");
        command.arg(&self.id);
        command
    }
}

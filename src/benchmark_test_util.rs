use crate::alpha::block;
use crate::alpha::block::genesis_vrf_out;
use crate::alpha::block::Block;
use crate::alpha::coinbase::CoinbaseOperation;
use crate::alpha::initial_staker::InitialStaker;
use crate::alpha::stake::StakeOperation;
use crate::alpha::state::State;
use crate::alpha::transfer::TransferOperation;
use crate::alpha::types::TxHash;
use crate::cell::inputs::{Input, Inputs};
use crate::cell::outputs::{Output, Outputs};
use crate::cell::types::{Capacity, PublicKeyHash};
use crate::cell::{Cell, CellType};
use crate::graph::dependency_graph::DependencyGraph;
use crate::graph::DAG;
use crate::sleet;
use crate::sleet::tx::Tx;
use crate::storage::tx as tx_storage;
use crate::zfx_id::Id;
use ed25519_dalek::Keypair;
use rand::rngs::OsRng;
use rand::thread_rng;
use sled::Db;
use std::collections::HashSet;
use std::convert::TryInto;

pub fn insert_into_dependency_graph(cells: Vec<Cell>) -> DependencyGraph {
    let mut graph = DependencyGraph::new();
    for cell in cells {
        graph.insert(cell);
    }
    graph
}

pub fn create_n_cells(n: u64) -> Vec<Cell> {
    let (keypair_1, _, pub_key_1, pub_key_2) = create_keys();
    let mut cells = vec![];

    let coinbase_op = CoinbaseOperation::new(vec![(pub_key_1.clone(), 10000000000000000)]);
    let mut transfer_tx = coinbase_op.try_into().unwrap();

    for i in 1..n {
        let transfer_op =
            TransferOperation::new(transfer_tx, pub_key_2.clone(), pub_key_1.clone(), i);
        transfer_tx = transfer_op.transfer(&keypair_1).unwrap();
        cells.push(transfer_tx.clone());
    }
    cells
}

pub fn create_n_cells_with_duplicates(n: u64) -> Vec<Cell> {
    let mut cells = create_n_cells(n);

    for cell in cells.clone() {
        for _ in 0..5 {
            cells.push(cell.clone());
        }
    }
    cells
}

pub fn insert_into_dag(tx_hashes: Vec<TxHash>) -> DAG<TxHash> {
    let mut dag: DAG<TxHash> = DAG::new();

    dag.insert_vx(tx_hashes[0], vec![]).unwrap();
    dag.insert_vx(tx_hashes[1], vec![tx_hashes[0]]).unwrap();
    for i in 2..tx_hashes.len() {
        dag.insert_vx(tx_hashes[i], vec![tx_hashes[i - 1], tx_hashes[i - 2]]).unwrap();
    }
    dag
}

pub fn make_db_inserts(mut db: &Db, n: u64) -> Vec<TxHash> {
    let (_, _, pub_key_1, _) = create_keys();
    let mut tx_hashes = vec![];

    for i in 1..n {
        let coinbase_op = CoinbaseOperation::new(vec![(pub_key_1.clone(), i)]);
        let tx = Tx::new(vec![], coinbase_op.try_into().unwrap());
        tx_storage::insert_tx(db, tx);
    }
    tx_hashes
}

pub fn get_transactions_from_db(mut db: &Db, tx_hashes: Vec<TxHash>) {
    for tx_hash in tx_hashes {
        tx_storage::get_tx(db, tx_hash);
    }
}

pub fn build_blocks(n: u64) -> Vec<Block> {
    let mut blocks = vec![];
    for staker in genesis_stakers(n) {
        // Aggregate the allocations into one coinbase output so that the conflict graph has one genesis
        // vertex.
        let mut allocations = vec![];
        let pkh = staker.public_key_hash().unwrap();
        allocations.push((pkh.clone(), staker.total_allocation.clone()));

        let allocations_op = CoinbaseOperation::new(allocations);
        let allocations_tx: Cell = allocations_op.try_into().unwrap();
        // Construct the genesis block.
        let mut cells = vec![];

        let pkh = staker.public_key_hash().unwrap();
        let stake_op = StakeOperation::new(
            allocations_tx.clone(),
            staker.node_id.clone(),
            pkh.clone(),
            staker.staked_allocation.clone(),
        );
        let stake_tx = stake_op.stake(&staker.keypair).unwrap();
        cells.push(stake_tx);
        cells.push(allocations_tx);
        blocks.push(Block {
            predecessor: None,
            height: 0u64,
            vrf_out: genesis_vrf_out().unwrap(),
            cells,
        });
    }
    blocks
}

pub fn genesis_stakers(n: u64) -> Vec<InitialStaker> {
    let mut stackers = vec![];
    for i in 0..n {
        let mut csprng = OsRng {};
        let keypair = Keypair::generate(&mut csprng);
        stackers.push(InitialStaker {
            keypair,
            node_id: Id::generate(),
            total_allocation: 2000 + i,
            staked_allocation: 1000 + i,
        });
    }
    stackers
}

pub fn create_keys() -> (Keypair, Keypair, PublicKeyHash, PublicKeyHash) {
    let keypair_hex_1 = "ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416".to_owned();
    let keypair_hex_2 = "5a353c630d3faf8e2d333a0983c1c71d5e9b6aed8f4959578fbeb3d3f3172886393b576de0ac1fe86a4dd416cf032543ac1bd066eb82585f779f6ce21237c0cd".to_owned();

    let keypair_1: Keypair = Keypair::from_bytes(&hex::decode(keypair_hex_1).unwrap()).unwrap();
    let keypair_2: Keypair = Keypair::from_bytes(&hex::decode(keypair_hex_2).unwrap()).unwrap();
    let pub_key_1: [u8; 32] = hash_public(&keypair_1);
    let pub_key_2: [u8; 32] = hash_public(&keypair_2);

    (keypair_1, keypair_2, pub_key_1, pub_key_2)
}

fn hash_public(keypair: &Keypair) -> [u8; 32] {
    let enc = bincode::serialize(&keypair.public).unwrap();
    blake3::hash(&enc).as_bytes().clone()
}

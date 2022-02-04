use crate::alpha::transfer::TransferOperation;
use crate::cell::types::{Capacity, CellHash, FEE};
use crate::client;
use crate::integration_test::test_functions::*;
use crate::integration_test::test_model::{IntegrationTestContext, TestNode, TestNodes};
use crate::protocol::{Request, Response};
use crate::sleet;
use crate::Result;
use std::thread::{sleep, JoinHandle};

use crate::alpha::block::Block;
use crate::alpha::types::{BlockHash, VrfOutput};
use crate::cell::CellType;
use crate::zfx_id::Id;
use std::time::{Duration, Instant};
use tracing::info;

pub async fn run_hail_integration_test() -> Result<()> {
    let mut nodes = TestNodes::new();

    nodes.start_all();
    wait_until_nodes_start(&nodes).await?;

    let from = nodes.get_node(0).unwrap();
    let to = nodes.get_node(1).unwrap();
    let mut cells_hashes = vec![*get_cell_hashes_with_max_capacity(from).await.get(0).unwrap()];
    let mut accepted_cell_hashes = vec![];

    // send bunch of cells to be able to verify those which are accepted in blocks
    for _ in 1..20 {
        sleep(Duration::from_secs(100));
        cells_hashes = send(from, to, 1 as Capacity, cells_hashes).await;
        accepted_cell_hashes.push(cells_hashes.get(0).unwrap().0);
    }

    sleep(Duration::from_secs(5));

    let mut previous_block: Option<Block> = None;
    // as the default confidence level is 11,
    // we expect 9 accepted blocks having 1 cell from first 9 transferred cells
    // according to the transferred order
    for i in 1..10 {
        if let Some(block) = get_block(from.address, i).await? {
            info!("Block height = {}", i);

            assert_eq!(i, block.height);
            assert_eq!(1, block.cells.len());
            assert_eq!(accepted_cell_hashes[i as usize - 1], block.cells[0].hash());

            if previous_block.is_some() {
                let block_ref = previous_block.unwrap();
                let previous_block_hash = block_ref.hash().unwrap();
                let expected_vrfs = get_expected_vrfs(&nodes, &block_ref);

                let contains = expected_vrfs.contains(&block.vrf_out);
                assert!(
                    contains,
                    format!("VRF for block wih height {} was not generated correctly", i)
                );
                assert_eq!(previous_block_hash, block.predecessor.unwrap());
            }
            previous_block = Some(block);
        }
    }

    Result::Ok(())
}

fn get_expected_vrfs(nodes: &TestNodes, block_ref: &Block) -> Vec<VrfOutput> {
    nodes
        .nodes
        .iter()
        .map(|n| {
            let node_id = Id::from_ip(&n.address);
            let vrf_h = vec![node_id.as_bytes(), &block_ref.vrf_out].concat();
            blake3::hash(&vrf_h).as_bytes().clone()
        })
        .collect::<Vec<VrfOutput>>()
}

async fn send(
    from: &TestNode,
    to: &TestNode,
    amount: Capacity,
    mut spendable_cell_hashes: Vec<(CellHash, Capacity)>,
) -> Vec<(CellHash, Capacity)> {
    let total_to_spend = amount + FEE;
    let mut updated_spendable_cell_hashes = spendable_cell_hashes.clone();
    if let Some((cell_hash, capacity)) =
        spendable_cell_hashes.iter().find(|(_, c)| *c > total_to_spend)
    {
        let spent_cell_hash = spend_cell(from, to, *cell_hash, amount).await.unwrap();

        let new_capacity = capacity - total_to_spend;
        updated_spendable_cell_hashes.retain(|(h, _)| h != cell_hash);
        updated_spendable_cell_hashes.push((spent_cell_hash, new_capacity));
    }
    updated_spendable_cell_hashes
}

async fn spend_cell(
    from: &TestNode,
    to: &TestNode,
    cell_hash: CellHash,
    amount: u64,
) -> Result<CellHash> {
    if let Some(cell) = get_cell_from_hash(cell_hash, from.address).await? {
        Ok(send_cell(from, to, cell, amount).await?.unwrap())
    } else {
        panic!("cell doesn't exist: {}", hex::encode(&cell_hash));
    }
}

use crate::alpha::transfer::TransferOperation;
use crate::cell::types::{Capacity, CellHash, FEE};
use crate::client;
use crate::integration_test::test_functions::*;
use crate::integration_test::test_model::{IntegrationTestContext, TestNode, TestNodes};
use crate::protocol::{Request, Response};
use crate::sleet;
use crate::Result;
use std::collections::HashSet;
use std::thread::{sleep, JoinHandle};

use crate::alpha::block::Block;
use crate::alpha::types::{BlockHash, VrfOutput};
use crate::cell::CellType;
use crate::zfx_id::Id;
use std::time::{Duration, Instant};
use tracing::info;

pub async fn run_hail_integration_test() -> Result<()> {
    let mut nodes = TestNodes::new();

    nodes.start_all_and_wait().await?;

    let last_block_height = test_successful_block_generation(&nodes).await?;
    test_transfer_failure_and_check_block_not_generated(&nodes, last_block_height).await?;

    nodes.kill_all();

    Result::Ok(())
}

async fn test_successful_block_generation(nodes: &TestNodes) -> Result<u64> {
    info!("Run successful hail test: Transfer balance n-times between nodes and validate blocks");

    let from = nodes.get_node(0).unwrap();
    let to = nodes.get_node(1).unwrap();
    let mut cells_hashes = vec![*get_cell_hashes_with_max_capacity(from).await.get(0).unwrap()];

    let result = spend_many_from_cell_hashes(
        &from,
        &to,
        1 as Capacity,
        20,
        Duration::from_millis(100),
        cells_hashes,
    )
    .await?;
    let mut accepted_cell_hashes = result.0;

    sleep(Duration::from_secs(3));

    let mut previous_block: Option<Block> = None;
    let mut last_block_height: u64 = 0;
    let mut block_cells = HashSet::new();

    // as the default confidence level is 11,
    // we expect 10 accepted blocks having 1 cell from first 10 transferred cells
    // according to the transferred order
    for i in 1..11 {
        if let Some(block) = get_block(from.address, i).await? {
            info!("Block height = {}", i);
            block.cells.iter().for_each(|c| {
                block_cells.insert(c.clone());
            });
            block.cells.iter().for_each(|c| {
                assert!(
                    accepted_cell_hashes.contains(&c.hash()),
                    format!("Block {} doesn't contain an expected cell hash", i)
                );
            });

            if previous_block.is_some() {
                let block_ref = previous_block.unwrap();
                let previous_block_hash = block_ref.hash().unwrap();
                // FIXME: uncomment when hail is working properly. In rare scenarios VRF can be different from expected
                /*let expected_vrfs = get_expected_vrfs(&nodes, &block_ref);

                assert!(
                    expected_vrfs.contains(&block.vrf_out),
                    format!("VRF for block wih height {} was not generated correctly", i)
                );*/
                //assert_eq!(previous_block_hash, block.predecessor.unwrap());
            }
            previous_block = Some(block);
            last_block_height = i;
        }
    }
    assert!(block_cells.len() <= 10);

    Result::Ok(last_block_height)
}

async fn test_transfer_failure_and_check_block_not_generated(
    nodes: &TestNodes,
    latest_block_height: u64,
) -> Result<()> {
    info!("Run unsuccessful block generation test: Transfer invalid cell and check block was not created");

    let from = nodes.get_node(0).unwrap();
    let to = nodes.get_node(1).unwrap();
    let cell_hash = *get_cell_hashes_with_max_capacity(from).await.get(0).unwrap();
    let amount = 20;

    let cell = get_cell_from_hash(cell_hash.0, from.address).await?.unwrap();
    let odd_transfer_op =
        TransferOperation::new(cell.clone(), Id::generate().bytes(), from.public_key, amount);
    let odd_transfer = odd_transfer_op.transfer(&from.keypair).unwrap();

    spend_cell(&from, &to, odd_transfer, amount).await?;

    // previous block was generated but next one shouldn't
    assert!(get_block(from.address, latest_block_height + 1).await?.is_none());

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

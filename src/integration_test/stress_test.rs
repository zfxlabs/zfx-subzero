use std::collections::HashSet;
use std::future::Future;
use std::net::SocketAddr;
use std::thread::sleep;
use std::time::Duration;

use crate::cell::Cell;
use futures_util::FutureExt;
use tokio::task::JoinHandle;
use tokio::time::Timeout;
use tracing::{error, info};

use crate::cell::types::{Capacity, CellHash, FEE};
use crate::integration_test::test_functions::*;
use crate::integration_test::test_model::{IntegrationTestContext, TestNode, TestNodes};
use crate::Result;

const ITERATION_LIMIT: u64 = 40;

pub async fn run_stress_test() -> Result<()> {
    info!("Run stress test: Transfer balance n-times from all 3 nodes in parallel");

    let mut nodes = TestNodes::new();
    nodes.start_all_and_wait().await?;

    let mut results_futures = vec![];
    results_futures.push(send(0, 1));
    results_futures.push(send(1, 2));
    results_futures.push(send(2, 0));

    let has_error = futures::future::join_all(results_futures)
        .map(|results| {
            let mut has_error = false;
            for r in results.iter() {
                if let Err(_) = r {
                    has_error = true
                }
            }
            has_error
        })
        .await;

    sleep(Duration::from_secs(5));

    // validate blocks and cells consistency across all nodes

    validate_blocks(&nodes).await;

    let cell_hashes = validate_cell_hashes(&mut nodes, |addr| get_cell_hashes(addr)).await?;
    assert_eq!((nodes.nodes.len() * ITERATION_LIMIT as usize + 4), cell_hashes.len());

    validate_cell_hashes(&mut nodes, |addr| get_accepted_cell_hashes(addr)).await?;

    assert!(!has_error, "Stress test failed as one of the thread got an error");

    nodes.kill_all();

    Result::Ok(())
}

async fn validate_cell_hashes<F, Fut>(
    nodes: &mut TestNodes,
    get_cell_hashes: F,
) -> Result<HashSet<CellHash>>
where
    F: Fn(SocketAddr) -> Fut,
    Fut: Future<Output = Result<Vec<CellHash>>>,
{
    let mut unique_cell_hashes = HashSet::new();
    let mut total_cell_hashes_numbers = HashSet::new();
    for n in &nodes.nodes {
        let cell_hashes = get_cell_hashes(n.address).await.unwrap();
        total_cell_hashes_numbers.insert(cell_hashes.len());
        cell_hashes.iter().for_each(|h| {
            unique_cell_hashes.insert(h.clone());
        });
    }

    assert_eq!(1, total_cell_hashes_numbers.len()); // check that it's the same size across all 3 nodes
    assert_eq!(unique_cell_hashes.len(), *total_cell_hashes_numbers.iter().last().unwrap());

    Ok(unique_cell_hashes)
}

async fn validate_blocks(nodes: &TestNodes) {
    info!("Validate blocks after stress test");

    let mut cells_in_blocks = vec![];
    for n in &nodes.nodes {
        let mut total_blocks = 1;
        let mut total_cells_in_blocks = 0;
        let mut cells_in_block = Vec::new();
        while let Some(block) = get_block(n.address, total_blocks).await.unwrap() {
            total_cells_in_blocks = total_cells_in_blocks + block.cells.len();
            block.cells.iter().for_each(|c| {
                cells_in_block.push(c.hash());
            });
            total_blocks = total_blocks + 1;
        }
        cells_in_blocks.push(cells_in_block);
        info!("total blocks = {}", total_blocks);
        info!("total cells = {}", total_cells_in_blocks);
    }
    info!("total cells in block = {}", cells_in_blocks.len());

    // FIXME: uncomment when hail is working properly
    // assert_eq!(total_cells_in_blocks, cells_in_blocks.len());
}

fn send(from_node_id: usize, to_node_id: usize) -> JoinHandle<Result<()>> {
    const AMOUNT: Capacity = 1 as Capacity;
    const FULL_AMOUNT: u64 = AMOUNT + FEE;

    let handle = tokio::spawn(async move {
        let test_nodes = TestNodes::new();
        let from = test_nodes.get_node(from_node_id).unwrap();
        let to = test_nodes.get_node(to_node_id).unwrap();

        // get max spendable amount and transfer iterations
        let spendable_cells = get_cell_hashes_with_max_capacity(from).await;
        let total_spendable_amount = spendable_cells.iter().map(|(_, r)| r).sum::<u64>();
        let residue_per_max_iterations: Vec<(u64, u64)> = spendable_cells
            .iter()
            .map(|(_, max_capacity)| {
                let iterations = (max_capacity / FULL_AMOUNT) as u64;
                let expected_residue = max_capacity - iterations * FULL_AMOUNT;
                (iterations, expected_residue as u64)
            })
            .collect::<Vec<(u64, u64)>>();
        let mut iterations: u64 = residue_per_max_iterations.iter().map(|(i, _)| i).sum::<u64>();

        // FIXME: temporal solution until the issue with DAG in sleet is fixed
        if iterations > ITERATION_LIMIT {
            iterations = ITERATION_LIMIT
        }

        let expected_balance = total_spendable_amount - iterations * FULL_AMOUNT;

        // start sending cells
        let transfer_result =
            spend_many(from, to, AMOUNT, iterations as usize, Duration::from_millis(10)).await?;
        sleep(Duration::from_secs(2));

        // validate the remaining balance and transferred cells
        let cell_hashes =
            get_cell_hashes(test_nodes.get_node(from_node_id).unwrap().address).await.unwrap();
        let mut transferred_balance = 0;
        let mut remaining_balance = 0;
        for cell_hash in transfer_result.0 {
            let cell = get_cell_from_hash(cell_hash, from.address).await?.unwrap();
            transferred_balance = transferred_balance
                + cell.outputs_of_owner(&to.public_key).iter().map(|o| o.capacity).sum::<u64>();

            assert!(cell_hashes.contains(&cell_hash)); // verify that all spent cells are in the node
        }

        for cell_hash in transfer_result.1 {
            let cell = get_cell_from_hash(cell_hash.0, from.address).await?.unwrap();
            remaining_balance = remaining_balance
                + cell.outputs_of_owner(&from.public_key).iter().map(|o| o.capacity).sum::<u64>();
        }

        assert_eq!(expected_balance, remaining_balance);
        assert_eq!(iterations, transferred_balance);

        Ok(())
    });
    handle
}

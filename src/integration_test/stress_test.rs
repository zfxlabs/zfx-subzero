use actix::{Actor, AsyncContext};
use actix_rt::{Arbiter, System};
use std::collections::HashSet;
use std::future::Future;
use std::net::SocketAddr;
use std::ops::Range;
use std::sync::mpsc::RecvError;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::{sleep, Thread};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::cell::Cell;
use futures_util::FutureExt;
use rand::{thread_rng, Rng};
use serde::de::Unexpected::Option;
use tokio::runtime;
use tokio::task::JoinHandle;
use tokio::time::Timeout;
use tracing::{debug, error, info};

use crate::cell::types::{Capacity, CellHash, FEE};
use crate::integration_test::test_functions::*;
use crate::integration_test::test_model::{IntegrationTestContext, TestNode, TestNodes};
use crate::integration_test::test_node_chaos_manager::TestNodeChaosManager;
use crate::Result;

pub async fn run_all_stress_tests() -> Result<()> {
    run_long_stress_test_with_valid_transfers().await?;
    sleep(Duration::from_secs(5));
    run_stress_test_with_valid_transfers().await?;
    sleep(Duration::from_secs(5));
    run_node_communication_stress_test().await?;
    sleep(Duration::from_secs(5));
    run_stress_test_with_failed_transfers().await?;

    Result::Ok(())
}

/// Run stress test by transferring valid cells among 3 nodes in parallel.
///
/// Verifies that all cells were transferred and stored in 'sleet'.
/// Verifies transfer and remaining balance in all nodes.
/// Verifies that blocks contains accepted cells in all 3 nodes are same and unique.
pub async fn run_stress_test_with_valid_transfers() -> Result<()> {
    info!("Run stress test: Transfer balance n-times from all 3 nodes in parallel");
    let transfer_delay = Duration::from_millis(50);
    let max_iterations = 40;

    let mut nodes = TestNodes::new();
    nodes.start_minimal_and_wait().await?;

    let initial_cells_size = get_total_initial_cell_hashes(&mut nodes).await?;

    let mut results_futures = vec![];
    results_futures.push(send(0, 1, transfer_delay, max_iterations));
    results_futures.push(send(1, 2, transfer_delay, max_iterations));
    results_futures.push(send(2, 0, transfer_delay, max_iterations));

    let has_error = wait_for_future_response(results_futures).await;

    sleep(Duration::from_secs(5));

    // validate blocks and cells consistency across all nodes
    // FIXME: uncomment when hail is working properly
    // validate_blocks(&nodes).await;

    let cell_hashes = validate_cell_hashes(&mut nodes, |addr| get_cell_hashes(addr)).await?;
    assert_eq!(
        (nodes.get_running_nodes().len() * max_iterations as usize + initial_cells_size),
        cell_hashes.len()
    );

    validate_cell_hashes(&mut nodes, |addr| get_accepted_cell_hashes(addr)).await?;

    assert!(!has_error, "Stress test failed as one of the thread got an error");

    nodes.kill_all();

    Result::Ok(())
}

/// Run a long stress test by transferring valid cells among 3 nodes in parallel.
///
/// Verifies that all cells were transferred and stored in 'sleet'.
pub async fn run_long_stress_test_with_valid_transfers() -> Result<()> {
    info!("Run long stress test: Transfer balance n-times from all 3 nodes in parallel");
    let transfer_delay = Duration::from_millis(50);
    let max_iterations = 700;

    let mut nodes = TestNodes::new();
    nodes.start_minimal_and_wait().await?;

    let mut results_futures = vec![];
    results_futures.push(send(0, 1, transfer_delay, max_iterations));
    results_futures.push(send(1, 2, transfer_delay, max_iterations));
    results_futures.push(send(2, 0, transfer_delay, max_iterations));

    let has_error = wait_for_future_response(results_futures).await;

    assert!(!has_error, "Stress test failed as one of the thread got an error");

    nodes.kill_all();

    Result::Ok(())
}

/// Run or stop n-number of nodes periodically for some time and
/// verify the status of each node - number of peers, validators and its weight
pub async fn run_node_communication_stress_test() -> Result<()> {
    info!("Run stress for node communication: Start and stop some nodes and check their status");

    let mut nodes = TestNodes::new();
    nodes.start_minimal_and_wait().await?;

    apply_node_actions(
        &mut nodes,
        vec![(3, true, 15), (1, false, 20), (1, true, 5), (4, true, 45)],
    );

    for node in &nodes.get_running_nodes() {
        let status = get_node_status(node.address).await?.unwrap();
        assert!(status.peers.len() >= 3);
        assert!(status.validators.len() >= 3);
        for validator in status.validators {
            // The weight will depend on stake of validators and currently is hardcoded to 2000 each
            // For total of 5 nodes with same stake, each validator should have 20% weight
            assert_eq!(0.2, validator.2);
        }
    }

    nodes.kill_all();

    Result::Ok(())
}

/// Transfer valid cells from one node to another when a random node can stop/start periodically.
/// The random node which stops must not affect the stability and reaching consensus for transactions.
///
/// Verifies that all cells were transferred successfully.
pub async fn run_stress_test_with_chaos() -> Result<()> {
    info!("Run stress test with chaos: Transfer balance n-times from all 3 nodes in parallel");

    let mut nodes = TestNodes::new();
    nodes.start_minimal_and_wait().await?;

    let mut manager = TestNodeChaosManager::new(
        Arc::new(Mutex::new(nodes)),
        Duration::from_secs(420),
        Range { start: 60, end: 90 },
        Range { start: 3, end: 4 },
    );
    manager.run_chaos();

    sleep(Duration::from_secs(20));

    let mut results_futures = vec![];
    results_futures.push(send(0, 1, Duration::from_secs(10), 100));

    let has_error = wait_for_future_response(results_futures).await;

    manager.stop();

    assert!(!has_error, "Stress test failed as one of the thread got an error");

    Result::Ok(())
}

/// Transfer valid and invalid cells in parallel from one node to another.
///
/// Validate that valid cells were transferred successfully and invalid cells are ignored.
pub async fn run_stress_test_with_failed_transfers() -> Result<()> {
    info!("Run stress test with failed transfers: Transfer valid and invalid cells n-times between 2 nodes in parallel");

    let mut nodes = TestNodes::new();
    nodes.start_minimal_and_wait().await?;

    let mut results_futures = vec![];
    // send traffic with valid and invalid transfers so they can intersect with each other
    results_futures.push(send_from_accepted_cells(0, 1, Duration::from_millis(100)));
    results_futures.push(send_from_invalid_cells(0, 1, Duration::from_millis(150)));
    results_futures.push(send(0, 1, Duration::from_millis(300), 40));

    let has_error = wait_for_future_response(results_futures).await;

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
    for n in &nodes.get_running_nodes() {
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
    let mut total_cells_in_blocks = 0;
    for n in &nodes.get_running_nodes() {
        let mut total_blocks = 1;
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

    assert_eq!(total_cells_in_blocks, cells_in_blocks.len());
}

fn send(
    from_node_id: usize,
    to_node_id: usize,
    transfer_delay: Duration,
    max_iterations: u64,
) -> JoinHandle<Result<()>> {
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

        if iterations > max_iterations {
            iterations = max_iterations
        }

        let expected_balance = total_spendable_amount - iterations * FULL_AMOUNT;

        // start sending cells
        let transfer_result =
            spend_many(from, to, AMOUNT, iterations as usize, transfer_delay).await?;
        sleep(Duration::from_secs(5));

        // validate the remaining balance and transferred cells
        let mut transferred_balance = 0;
        let mut remaining_balance = 0;
        for cell_hash in transfer_result.0 {
            let found_cell = get_cell_from_hash(cell_hash, from.address).await?;
            if let Some(cell) = found_cell {
                transferred_balance = transferred_balance
                    + cell.outputs_of_owner(&to.public_key).iter().map(|o| o.capacity).sum::<u64>();
            } else {
                error!("Failed to find cell for hash {}", hex::encode(cell_hash));
            }
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

/// Attempt to spend and send already accepted cells to generate a traffic of invalid cells.
///
/// For each transfer, validates that it was not successful
fn send_from_accepted_cells(
    from_node_id: usize,
    to_node_id: usize,
    transfer_delay: Duration,
) -> JoinHandle<Result<()>> {
    let handle = tokio::spawn(async move {
        let test_nodes = TestNodes::new();
        let from = test_nodes.get_node(from_node_id).unwrap();
        let to = test_nodes.get_node(to_node_id).unwrap();

        sleep(Duration::from_millis(1000)); // delay a bit until accepted cells appear

        spend_many_from_accepted_cells(from, to, 30, transfer_delay).await?;

        Ok(())
    });
    handle
}

/// Attempt to spend and send invalid cells.
///
/// For each transfer, validates that it was not successful
fn send_from_invalid_cells(
    from_node_id: usize,
    to_node_id: usize,
    transfer_delay: Duration,
) -> JoinHandle<Result<()>> {
    let handle = tokio::spawn(async move {
        let test_nodes = TestNodes::new();
        let from = test_nodes.get_node(from_node_id).unwrap();
        let to = test_nodes.get_node(to_node_id).unwrap();

        spend_many_from_invalid_cells(from, to, 40, transfer_delay).await?;

        Ok(())
    });
    handle
}

fn apply_node_actions(nodes: &mut TestNodes, node_actions: Vec<(usize, bool, u64)>) {
    for (id, is_start, delay) in node_actions {
        if is_start {
            nodes.start_node(id);
        } else {
            nodes.kill_node(id);
        }
        sleep(Duration::from_secs(delay));
    }
}

async fn wait_for_future_response(mut results_futures: Vec<JoinHandle<Result<()>>>) -> bool {
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
    has_error
}

async fn get_total_initial_cell_hashes(nodes: &mut TestNodes) -> Result<usize> {
    let mut initial_cells_len = 0;
    for node in nodes.get_running_nodes() {
        let cell_hashes = get_cell_hashes(node.address).await?;
        if cell_hashes.len() > initial_cells_len {
            initial_cells_len = cell_hashes.len();
        }
    }
    Ok(initial_cells_len)
}

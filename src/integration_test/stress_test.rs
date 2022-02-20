use std::collections::HashSet;
use std::thread::sleep;
use std::time::Duration;

use crate::cell::Cell;
use futures_util::FutureExt;
use tokio::task::JoinHandle;
use tokio::time::{timeout, Timeout};
use tracing::info;

use crate::cell::types::{Capacity, FEE};
use crate::integration_test::test_functions::*;
use crate::integration_test::test_model::{IntegrationTestContext, TestNode, TestNodes};
use crate::Result;

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

    validate_blocks(&nodes).await;

    nodes.kill_all();

    assert!(!has_error, "Stress test failed as one of the thread got an error");

    Result::Ok(())
}

async fn validate_blocks(nodes: &TestNodes) {
    info!("Validate blocks after stress test");

    let mut total_blocks = 0;
    let mut total_cells_in_blocks = 0;
    for n in &nodes.nodes {
        let mut height = 1;
        while let Some(block) = get_block(n.address, height).await.unwrap() {
            total_cells_in_blocks = total_cells_in_blocks + block.cells.len();
            height = height + 1;
        }
        total_blocks = total_blocks + height;
    }
    info!("total blocks = {}", total_blocks);
    info!("total cells = {}", total_cells_in_blocks);

    // FIXME: uncomment when hail is working properly
    // assert_eq!(total_cells_in_blocks, cells_in_blocks.len());
}

fn send(from_node_id: usize, to_node_id: usize) -> JoinHandle<Result<()>> {
    const AMOUNT: Capacity = 1 as Capacity;
    const FULL_AMOUNT: u64 = AMOUNT + FEE;
    const ITERATION_LIMIT: u64 = 40;

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
        let mut iterations : u64 = residue_per_max_iterations.iter().map(|(i, _)| i).sum::<u64>();

        // FIXME: temporal solution until the issue with DAG in sleet is fixed
        if iterations > ITERATION_LIMIT {
            iterations = ITERATION_LIMIT
        }

        let expected_balance = total_spendable_amount - iterations * FULL_AMOUNT;

        // start sending cells
        let mut transfer_result =
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

            assert!(cell_hashes.contains(&cell_hash));   // verify that all spent cells are in the node
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

async fn send_with_timeout(
    from: &TestNode,
    to: &TestNode,
    amount: u64,
    context: &mut IntegrationTestContext,
) -> Option<Cell> {
    let mut attempts = 5;
    while attempts > 0 {
        match timeout(Duration::from_millis(1000), send_and_check_cell(from, to, amount, context))
            .await
        {
            Ok(r) => match r {
                Ok(c) => return Some(c),
                _ => {}
            },
            Err(_) => {
                info!("Failed to send within timeout. Attempt = {}", attempts)
            }
        }
        attempts -= 1
    }
    return None;
}

async fn send_and_check_cell(
    from: &TestNode,
    to: &TestNode,
    amount: u64,
    context: &mut IntegrationTestContext,
) -> Result<Cell> {
    sleep(Duration::from_millis(100)); // make a controlled delay between transfers

    let cell = get_cell(amount, context, from).await?.unwrap();
    let cell_hash = cell.hash();
    let previous_output_len = cell.outputs().len();

    let spent_cell_hash = send_cell(from, to, cell, amount).await?;
    assert!(spent_cell_hash.is_some());
    let spent_cell = get_cell_from_hash(spent_cell_hash.unwrap(), from.address).await?;
    assert!(spent_cell.is_some());
    let spent_cell_outputs_len = spent_cell.as_ref().unwrap().outputs().len();

    register_cell_in_test_context(
        cell_hash,
        spent_cell_hash.unwrap(),
        spent_cell_outputs_len,
        previous_output_len,
        context,
    );

    Result::Ok(spent_cell.unwrap())
}

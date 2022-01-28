use std::thread::sleep;
use std::time::Duration;

use crate::cell::Cell;
use futures_util::FutureExt;
use tokio::task::JoinHandle;
use tokio::time::{timeout, Timeout};
use tracing::info;

use crate::cell::types::FEE;
use crate::integration_test::test_functions::*;
use crate::integration_test::test_model::{IntegrationTestContext, TestNode, TestNodes};
use crate::Result;

pub async fn run_integration_stress_test() -> Result<()> {
    let mut results_futures = vec![];
    results_futures.push(send(0, 1));
    results_futures.push(send(1, 2));
    results_futures.push(send(2, 0));

    let has_error = futures::future::join_all(results_futures)
        .map(|results| {
            let mut has_error = false;
            for r in results.iter() {
                match r {
                    Err(_) => has_error = true,
                    _ => {}
                }
            }
            has_error
        })
        .await;

    assert!(!has_error, "Stress test failed");

    Result::Ok(())
}

fn send(from_node_id: usize, to_node_id: usize) -> JoinHandle<Result<()>> {
    const AMOUNT: u64 = 1;
    const FULL_AMOUNT: u64 = AMOUNT + FEE;
    const ITERATION_LIMIT: u64 = 40;

    let handle = tokio::spawn(async move {
        let mut context = IntegrationTestContext::new();
        let test_nodes = TestNodes::new();
        let from = test_nodes.get_node(from_node_id).unwrap();
        let to = test_nodes.get_node(to_node_id).unwrap();

        let cells_of_node = get_cell_outputs_of_node(from, &mut context).await.unwrap();
        let residue_per_max_iterations: Vec<(u64, u64)> = cells_of_node
            .iter()
            .map(|c| {
                info!("capacity = {}", c.capacity);
                let iterations = (c.capacity / FULL_AMOUNT) as u64;
                let expected_residue = c.capacity - iterations * FULL_AMOUNT;
                (iterations, expected_residue as u64)
            })
            .collect::<Vec<(u64, u64)>>();
        let mut iterations = residue_per_max_iterations.iter().map(|(i, _)| i).sum::<u64>();

        // FIXME: temporal solution until the issue with DAG in sleet is fixed
        if iterations > ITERATION_LIMIT {
            iterations = ITERATION_LIMIT
        }
        let expected_balance =
            cells_of_node.iter().map(|o| o.capacity).sum::<u64>() - iterations * FULL_AMOUNT;

        let mut transferred_cells: Vec<Cell> = vec![];
        for i in 1..iterations + 1 {
            transferred_cells
                .push(send_with_timeout(from, to, AMOUNT, &mut context).await.unwrap());
            info!("Iteration = {}", i);
        }

        let remaining_balance =
            get_cell_outputs_of_node(test_nodes.get_node(from_node_id).unwrap(), &mut context)
                .await
                .unwrap()
                .iter()
                .map(|o| o.capacity)
                .filter(|c| *c != AMOUNT)
                .sum::<u64>();
        assert_eq!(expected_balance, remaining_balance);

        let cell_hashes =
            get_cell_hashes(test_nodes.get_node(from_node_id).unwrap().address).await.unwrap();
        info!("Total hashes = {}", cell_hashes.len());
        for cell in transferred_cells {
            assert!(cell_hashes.contains(&cell.hash()));
        }

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
        match timeout(Duration::from_millis(500), send_and_check_cell(from, to, amount, context))
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
    sleep(Duration::from_millis(10)); // make a controlled delay between transfers

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

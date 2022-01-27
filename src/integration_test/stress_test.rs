use std::borrow::{Borrow, BorrowMut};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread::{sleep, Thread};
use std::time::Duration;

use futures_util::FutureExt;
use tokio::task::JoinHandle;
use tracing::info;

use crate::alpha::coinbase::CoinbaseOperation;
use crate::alpha::stake::StakeOperation;
use crate::alpha::transfer::TransferOperation;
use crate::alpha::Error;
use crate::cell::inputs::Input;
use crate::cell::outputs::{Output, Outputs};
use crate::cell::types::{CellHash, PublicKeyHash, FEE};
use crate::cell::Cell;
use crate::ice::Status;
use crate::integration_test::test_functions::*;
use crate::integration_test::test_model::{IntegrationTestContext, TestNode, TestNodes};
use crate::zfx_id::Id;
use crate::Result;

pub async fn run_integration_stress_test() -> Result<()> {
    let mut results_futures = vec![];
    results_futures.push(send(0, 1));
    results_futures.push(send(1, 2));
    results_futures.push(send(2, 0));

    let results = futures::future::join_all(results_futures)
        .map(|results| {
            let mut responses: Vec<(Vec<Output>, Vec<u64>, u64)> = vec![];
            for r in results.iter() {
                match r {
                    Ok(inner) => match inner {
                        Ok(response) => responses.push(response.clone()),
                        Err(e) => {
                            info!("error {}", e)
                        }
                    },
                    Err(e) => {
                        info!("error {}", e)
                    }
                }
            }
            responses
        })
        .await;

    for result in results.iter() {
        let outputs = &result.0;
        let total_capacity = outputs.iter().map(|o| o.capacity).sum::<u64>();
        assert_eq!(total_capacity, result.2);
        // FIXME: uncomment when the issue with DAG in sleet is fixed
        /*for expected_residue in result.1.iter() {
            assert!(
                outputs.iter().find(|o| o.capacity == *expected_residue).is_some(),
                format!("No outputs have expected residue of {}", expected_residue)
            );
        }*/
    }

    Result::Ok(())
}

fn send(
    from_node_id: usize,
    to_node_id: usize,
) -> JoinHandle<Result<(Vec<Output>, Vec<u64>, u64)>> {
    const AMOUNT: u64 = 1;
    const FULL_AMOUNT: u64 = AMOUNT + FEE;
    const ITERATION_LIMIT: u64 = 50;

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
        let expected_residues =
            residue_per_max_iterations.iter().map(|(_, r)| *r).collect::<Vec<u64>>();

        // FIXME: temporal solution until the issue with DAG in sleet is fixed
        if iterations > ITERATION_LIMIT {
            iterations = ITERATION_LIMIT
        }
        let expected_total_residue = iterations
            + cells_of_node.iter().map(|o| o.capacity).sum::<u64>()
            - iterations * FULL_AMOUNT;

        for i in 1..iterations + 1 {
            test_send_cells_from_random_nodes(AMOUNT, from, to, &mut context).await;
            info!("Iteration = {}", i);
        }

        sleep(Duration::from_secs(10)); // wait some time so the nodes synchronize
        Ok((
            get_cell_outputs_of_node(test_nodes.get_node(from_node_id).unwrap(), &mut context)
                .await
                .unwrap(),
            expected_residues,
            expected_total_residue,
        ))
    });
    handle
}

async fn test_send_cells_from_random_nodes(
    amount: u64,
    from: &TestNode,
    to: &TestNode,
    context: &mut IntegrationTestContext,
) -> Result<()> {
    sleep(Duration::from_millis(10)); // make a controlled delay between transfers

    let cell = get_cell(amount, context, from).await?.unwrap();
    let cell_hash = cell.hash();
    let previous_output_len = cell.outputs().len();

    let spent_cell_hash = send_cell(from, to, cell, amount).await?;
    assert!(spent_cell_hash.is_some());
    let spent_cell = get_cell_from_hash(spent_cell_hash.unwrap(), from.address).await?;
    let spent_cell_outputs_len = spent_cell.as_ref().unwrap().outputs().len();

    register_cell_in_test_context(
        cell_hash,
        spent_cell_hash.unwrap(),
        spent_cell_outputs_len,
        previous_output_len,
        context,
    );

    Result::Ok(())
}

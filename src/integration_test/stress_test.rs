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
use futures_util::FutureExt;
use std::borrow::{Borrow, BorrowMut};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread::{sleep, Thread};
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::info;

pub async fn run_integration_stress_test() -> Result<()> {
    let mut results_futures = vec![];
    results_futures.push(send(0, 1));
    results_futures.push(send(1, 2));
    results_futures.push(send(2, 0));

    let results = futures::future::join_all(results_futures)
        .map(|results| {
            let mut responses: Vec<Vec<Output>> = vec![];
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

    for outputs in results.iter() {
        assert!(outputs.iter().find(|o| o.capacity == 91).is_some());
        assert!(outputs.iter().find(|o| o.capacity == 81).is_some());
    }

    Result::Ok(())
}

fn send(from_node_id: usize, to_node_id: usize) -> JoinHandle<Result<Vec<Output>>> {
    const MAX_SEND_ITERATIONS: i32 = 29;
    const AMOUNT: u64 = 1;

    let handle = tokio::spawn(async move {
        let mut context = IntegrationTestContext::new();
        let test_nodes = TestNodes::new();
        for _ in 1..MAX_SEND_ITERATIONS {
            let from = test_nodes.get_node(from_node_id).unwrap();
            let to = test_nodes.get_node(to_node_id).unwrap();
            test_send_cells_from_random_nodes(AMOUNT, from, to, &mut context).await;
        }
        get_cell_outputs_of_node(test_nodes.get_node(from_node_id).unwrap(), &mut context).await
    });
    handle
}

async fn test_send_cells_from_random_nodes(
    amount: u64,
    from: &TestNode,
    to: &TestNode,
    context: &mut IntegrationTestContext,
) -> Result<()> {
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

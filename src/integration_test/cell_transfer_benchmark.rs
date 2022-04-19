use std::thread::sleep;
use std::time::{Duration, Instant};

use futures_util::FutureExt;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tracing::info;

use crate::cell::types::{CellHash, FEE};
use crate::integration_test::test_functions::*;
use crate::integration_test::test_model::{TestNode, TestNodes};
use crate::Result;

const TRANSFER_DELAY: u64 = 10;

/// Run a performance test involving parallel cell transfers among 3 nodes.
/// Records time of each cell transfer and verifies min, max and avg time.
///
/// NOTE: the performance of cell transfers is run on local machine which varies in hardware
/// thus the timings can be different. This test is intended to capture a large performance degradation.
pub async fn run_cell_transfer_benchmark_test() -> Result<()> {
    info!("Run benchmark test for transfer cells: Transfer balance n-times from all 3 nodes in parallel");

    let mut nodes = TestNodes::new();
    nodes.start_minimal_and_wait().await?;

    run_cell_transfer_benchmark().await?;

    nodes.kill_all();
    Result::Ok(())
}

pub async fn run_cell_transfer_benchmark() -> Result<()> {
    let mut results_futures = vec![];
    results_futures.push(send(0, 1));
    results_futures.push(send(2, 0));
    results_futures.push(send(1, 2));

    match timeout(
        Duration::from_secs(60),
        futures::future::join_all(results_futures).map(|results| {
            let mut elapsed_times = vec![];
            for r in results.iter() {
                if let Ok(e) = r {
                    elapsed_times.push(e.clone())
                }
            }
            elapsed_times
        }),
    )
    .await
    {
        Ok(elapsed_times) => {
            let min = elapsed_times.iter().flatten().min().unwrap();
            let max = elapsed_times.iter().flatten().max().unwrap();
            let avg = (*max + *min) / 2;

            info!("Min = {:.2?}, Max = {:.2?}, Avg = {:.2?}", min, max, avg);
            assert!(
                avg.as_millis() < 80,
                "Average cell processing time took too long: {:.2?}",
                avg
            );
        }
        Err(_) => {
            panic!("Failed to finish benchmark test within the timeout")
        }
    }

    Result::Ok(())
}

fn send(from_node_id: usize, to_node_id: usize) -> JoinHandle<Vec<Duration>> {
    const ITERATION_LIMIT: u64 = 50;
    const AMOUNT: u64 = 1;

    let handle = tokio::spawn(async move {
        let test_nodes = TestNodes::new();
        let from = test_nodes.get_node(from_node_id).unwrap();
        let to = test_nodes.get_node(to_node_id).unwrap();

        let mut initial_cells_hashes = get_cell_hashes_with_max_capacity(from).await;

        let mut elapsed_time: Vec<Duration> = vec![];
        let mut updated_spendable_cell_hashes = initial_cells_hashes.clone();
        for i in 1..ITERATION_LIMIT + 1 {
            if let Some((cell_hash, capacity)) =
                initial_cells_hashes.iter_mut().find(|(_, c)| *c > i + FEE)
            {
                let (spent_cell_hash, elapsed) =
                    spend_cell_from_hash(from, to, *cell_hash, i).await.unwrap();

                updated_spendable_cell_hashes.retain(|(h, _)| h != cell_hash);
                updated_spendable_cell_hashes.push((spent_cell_hash, *capacity - i + FEE));
                initial_cells_hashes = updated_spendable_cell_hashes.clone();

                elapsed_time.push(elapsed);
            } else {
                break;
            }
        }

        elapsed_time
    });
    handle
}

async fn spend_cell_from_hash(
    from: &TestNode,
    to: &TestNode,
    cell_hash: CellHash,
    amount: u64,
) -> Result<(CellHash, Duration)> {
    sleep(Duration::from_millis(TRANSFER_DELAY));
    if let Some(cell) = get_cell_from_hash(cell_hash, from.address).await? {
        let now = Instant::now();
        let spent_cell = spend_cell(from, to, cell, amount).await?.unwrap();
        Ok((spent_cell, now.elapsed()))
    } else {
        panic!("cell doesn't exist: {}", hex::encode(&cell_hash));
    }
}

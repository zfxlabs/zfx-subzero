use std::panic;
use std::thread::sleep;
use std::time::Duration;
use tokio::time::timeout;
use tracing::info;

use crate::alpha::transfer::TransferOperation;
use crate::cell::inputs::Input;
use crate::cell::outputs::{Output, Outputs};
use crate::cell::types::{CellHash, FEE};
use crate::cell::Cell;
use crate::integration_test::test_functions::*;
use crate::integration_test::test_model::{IntegrationTestContext, TestNode, TestNodes};
use crate::zfx_id::Id;
use crate::Result;

const TRANSFER_RUN_TIMES: i32 = 5;

pub async fn run_all_integration_tests() -> Result<()> {
    let mut context = IntegrationTestContext::new();
    let mut nodes = TestNodes::new();
    nodes.start_all();
    wait_until_nodes_start(&nodes).await?;

    for _ in 0..TRANSFER_RUN_TIMES {
        test_send_cell(&nodes, &mut context).await?;
    }

    test_send_cell_with_modified_owner(&nodes, &mut context).await?;
    test_send_same_cell_twice(&nodes, &mut context).await?;
    test_send_cell_to_recipient_with_random_key(&nodes, &mut context).await?;
    test_send_cell_to_non_existing_recipient(&nodes, &mut context).await?;
    test_spend_unspendable_cell(&nodes, &mut context).await?;
    test_send_cell_when_has_faulty_node(&mut nodes, &mut context).await?;

    nodes.kill_all();
    Result::Ok(())
}

/// Transfer balance from one node to another
/// and validate its content
pub async fn test_send_cell(nodes: &TestNodes, context: &mut IntegrationTestContext) -> Result<()> {
    info!("Run test_send_cell: Transfer balance from one node to another");

    let from = nodes.get_node(0).unwrap();
    let to = nodes.get_node(1).unwrap();
    let spend_amount = 10 + context.test_run_counter as u64; // send diff amount to avoid duplicated txs

    let result = send_cell_and_get_result(from, to, spend_amount, nodes, context).await?;

    assert_cell(
        result.spent_cell,
        result.original_cell_hash,
        result.original_cell_output_len,
        result.original_cell_balance,
        spend_amount,
        from,
        to,
        context,
    );

    context.count_test_run();

    Result::Ok(())
}

/// Transfer balance to un-spendable cell,
/// the one which had been already spent earlier
/// and validate that it didn't go through
pub async fn test_spend_unspendable_cell(
    nodes: &TestNodes,
    context: &mut IntegrationTestContext,
) -> Result<()> {
    info!(
        "Run test_spend_unspendable_cell: Transfer balance to a cell which had been spent earlier"
    );

    let from = nodes.get_node(0).unwrap();
    let to = nodes.get_node(1).unwrap();
    let spend_amount = 42;

    // spend something first to make sure we have at least 1 un-spendable cell
    send_cell_and_get_result(from, to, spend_amount, nodes, context).await?;

    // get un-spendable cell and try to spend it
    let cell = get_not_spendable_cell(spend_amount + 1, context, from).await?.unwrap();
    let spent_cell_hash = send_cell(&from, &to, cell, spend_amount + 1).await?;

    if spent_cell_hash.is_some() {
        let spent_cell = get_cell_from_hash(spent_cell_hash.unwrap().clone(), from.address).await?;
        assert!(spent_cell.is_none())
    }

    context.count_test_run();
    Result::Ok(())
}

/// Transfer the same balance 2 times
/// and validate that it fails the second time
pub async fn test_send_same_cell_twice(
    nodes: &TestNodes,
    context: &mut IntegrationTestContext,
) -> Result<()> {
    info!("Run test_send_same_cell_twice: Transfer the same balance 2 times");

    let from = nodes.get_node(0).unwrap();
    let to = nodes.get_node(1).unwrap();
    let spend_amount: u64 = 3;

    let result = send_cell_and_get_result(from, to, spend_amount, nodes, context).await?;

    let same_cell = get_cell_from_hash(result.original_cell_hash, from.address).await?.unwrap();
    assert!(send_cell(&from, &to, same_cell.clone(), spend_amount).await?.is_none()); // check the duplicated cell was rejected

    context.count_test_run();
    Result::Ok(())
}

/// Transfer balance with modified recipient public key
/// and verify that transaction fails
pub async fn test_send_cell_to_recipient_with_random_key(
    nodes: &TestNodes,
    context: &mut IntegrationTestContext,
) -> Result<()> {
    info!("Run test_send_cell_with_invalid_hash: Transfer balance to node with random public key");

    let from = nodes.get_node(0).unwrap();
    let to = nodes.get_node(1).unwrap();
    let spend_amount = 5 as u64;

    let cell = get_cell(spend_amount, context, from).await?.unwrap();
    let odd_transfer_op =
        TransferOperation::new(cell.clone(), Id::generate().bytes(), from.public_key, spend_amount);
    let odd_transfer = odd_transfer_op.transfer(&from.keypair).unwrap();

    let spent_cell_hash = send_cell(&from, &to, odd_transfer, spend_amount).await?;
    assert!(spent_cell_hash.is_none());

    context.count_test_run();

    Result::Ok(())
}

pub async fn test_send_cell_with_modified_owner(
    nodes: &TestNodes,
    context: &mut IntegrationTestContext,
) -> Result<()> {
    info!(
        "Run test_send_cell_back_from_recipient_with_more_amount: \
        Return back balance when not sufficient funds"
    );

    let from = nodes.get_node(0).unwrap();
    let to = nodes.get_node(1).unwrap();
    let spend_amount = 23 as u64;

    let result = send_cell_and_get_result(from, to, spend_amount, nodes, context).await?;
    let new_inputs = result.spent_cell.inputs().clone();
    // make new outputs to have the same owner
    let new_outputs = result
        .spent_cell
        .outputs()
        .clone()
        .iter()
        .map(|o| {
            if o.lock == from.public_key {
                o.clone()
            } else {
                Output {
                    capacity: o.capacity,
                    cell_type: o.cell_type.clone(),
                    data: o.data.clone(),
                    lock: from.public_key,
                }
            }
        })
        .collect::<Vec<Output>>();
    let new_cell = Cell::new(new_inputs, Outputs { outputs: new_outputs });

    assert!(send_cell(&from, &to, new_cell, spend_amount - 1).await?.is_none());

    context.count_test_run();

    Result::Ok(())
}

/// Transfer balance to non-existing recipient
/// and check it was successful because a transfer can be made to any valid public key
pub async fn test_send_cell_to_non_existing_recipient(
    nodes: &TestNodes,
    context: &mut IntegrationTestContext,
) -> Result<()> {
    info!(
        "Run test_send_cell_to_non_existing_recipient: Transfer balance to non-existing recipient"
    );

    let from = nodes.get_node(0).unwrap();
    let non_existing_node = nodes.get_non_existing_node();
    let spend_amount = 65 as u64;

    send_cell_and_get_result(&from, &non_existing_node, spend_amount, nodes, context).await?;

    context.count_test_run();

    Result::Ok(())
}

/// Try to send a transfer when 1 node is down
/// and validate that transfer was not successful
pub async fn test_send_cell_when_has_faulty_node(
    nodes: &mut TestNodes,
    context: &mut IntegrationTestContext,
) -> Result<()> {
    info!("Run test_send_cell_when_has_faulty_node: Transfer balance when 1 node is down");

    nodes.kill_node(1);

    sleep(Duration::from_secs(10)); // wait some time so the network status updates

    let amount = 34;
    let from = &nodes.nodes[0];
    let to = &nodes.nodes[2];
    let cell = get_cell(amount, context, from).await?.unwrap();

    let spent_cell_hash = send_cell(from, to, cell, amount).await?;
    assert!(spent_cell_hash.is_some());

    assert_cell_presence_in_all_running_nodes(spent_cell_hash.unwrap(), false, nodes).await?;

    context.count_test_run();

    Result::Ok(())
}

pub async fn send_cell_and_get_result(
    from: &TestNode,
    to: &TestNode,
    amount: u64,
    nodes: &TestNodes,
    context: &mut IntegrationTestContext,
) -> Result<SendCellResult> {
    let cell = get_cell(amount, context, from).await?.unwrap();
    let cell_hash = cell.hash();
    let previous_output_len = cell.outputs().len();
    let previous_balance = get_outputs_capacity_of_owner(&cell, from);

    let spent_cell_hash = send_cell(from, to, cell, amount).await?;
    assert!(spent_cell_hash.is_some());

    // check that same tx was registered in all nodes
    let spent_cell =
        assert_cell_presence_in_all_running_nodes(spent_cell_hash.unwrap(), true, nodes).await?;

    let spent_cell_outputs = spent_cell.as_ref().unwrap().outputs();
    assert!(spent_cell_outputs.iter().find(|o| { o.capacity == amount }).is_some()); // check if transfer was successful

    register_cell_in_test_context(
        cell_hash,
        spent_cell_hash.unwrap(),
        spent_cell_outputs.len(),
        previous_output_len,
        context,
    );

    Ok(SendCellResult {
        original_cell_balance: previous_balance,
        original_cell_output_len: previous_output_len,
        original_cell_hash: cell_hash,
        spent_cell: spent_cell.unwrap(),
    })
}

/// Verify that all running nodes have a cell with
/// particular hash. Runs several attempts before can fail.
pub async fn assert_cell_presence_in_all_running_nodes(
    spent_cell_hash: CellHash,
    check_is_present: bool,
    nodes: &TestNodes,
) -> Result<Option<Cell>> {
    let mut spent_cell: Option<Cell> = None;
    let mut spent_cells_counter = 0;
    let running_nodes = &nodes.get_running_nodes();
    let nodes_len = running_nodes.len();
    let mut attempts = 3;

    while attempts > 0 {
        for node in running_nodes {
            if let Ok(Ok(c)) = timeout(
                Duration::from_secs(2),
                get_cell_from_hash(spent_cell_hash.clone(), node.address),
            )
            .await
            {
                spent_cell = c;
                if (check_is_present && spent_cell.is_some())
                    || (!check_is_present && spent_cell.is_none())
                {
                    spent_cells_counter += 1;
                }
            }
        }

        if spent_cells_counter == nodes_len {
            break;
        } else {
            spent_cells_counter = 0;
            attempts -= 1;
        }
    }

    assert_eq!(nodes_len, spent_cells_counter, "Not all running nodes have the spent cell");

    Ok(spent_cell)
}

fn assert_cell(
    spent_cell: Cell,
    cell_hash: CellHash,
    previous_len: usize,
    previous_balance: u64,
    spend_amount: u64,
    from: &TestNode,
    to: &TestNode,
    context: &mut IntegrationTestContext,
) {
    let spent_cell_hash = spent_cell.hash();
    let spent_cell_inputs = &spent_cell.inputs();
    let spent_cell_outputs = &spent_cell.outputs();
    let spent_cell_len = spent_cell_outputs.len();

    // validate outputs
    if spent_cell_len > 1 {
        assert_eq!(2, spent_cell_len, "Cell must have spent and remaining outputs");

        let remaining_output = spent_cell_outputs.iter().find(|o| o.lock == from.public_key);
        assert!(remaining_output.is_some(), "The remaining output doesn't exist");
        assert_eq!(
            previous_balance - FEE - spend_amount,
            remaining_output.unwrap().capacity,
            "Invalid balance of the remaining output"
        );
    } else {
        assert_eq!(1, spent_cell_len, "Cell must have only spent output");
    }
    let spent_output = spent_cell_outputs.iter().find(|o| o.lock == to.public_key);
    assert!(spent_output.is_some(), "The spent output doesn't exist");
    assert_eq!(spend_amount, spent_output.unwrap().capacity, "Invalid balance of the spent output");

    // validate inputs
    // assert_eq!(previous_len, spent_cell_inputs.len());
    let mut inputs_as_vec = spent_cell_inputs.inputs.iter().cloned().collect::<Vec<Input>>();
    inputs_as_vec.sort();
    let mut i = 0;
    for input in inputs_as_vec {
        assert_eq!(
            i as u8, input.output_index.index,
            "Cell input index must be always 0 as we have a single output to spend"
        );
        assert_eq!(
            cell_hash, input.output_index.cell_hash,
            "Invalid source (parent) of cell from which we consume amount"
        );
        assert_eq!(
            from.keypair.public.as_bytes(),
            input.unlock.public_key.as_bytes(),
            "Invalid cell owner in the input"
        );
        i += 1;
    }

    register_cell_in_test_context(
        cell_hash,
        spent_cell_hash,
        spent_cell_len,
        previous_len,
        context,
    );
}

pub struct SendCellResult {
    original_cell_balance: u64,
    original_cell_output_len: usize,
    original_cell_hash: CellHash,
    spent_cell: Cell,
}

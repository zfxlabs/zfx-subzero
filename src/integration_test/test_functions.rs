use std::collections::HashSet;
use std::net::SocketAddr;
use std::thread::sleep;
use std::time::Duration;

use rand::seq::SliceRandom;
use rand::thread_rng;

use crate::alpha::transfer::TransferOperation;
use crate::cell::types::{CellHash, PublicKeyHash};
use crate::cell::{Cell, CellType};
use crate::ice::Status;
use crate::integration_test::test_model::{IntegrationTestContext, TestNode, TestNodes};
use crate::protocol::Response;
use crate::Result;
use crate::{client, sleet, Request};

/// Register cell and it's parent to identify later which tx can be spent
pub fn register_cell_in_test_context(
    original_cell_hash: CellHash,
    spent_cell_hash: CellHash,
    spent_cell_output_len: usize,
    original_cell_output_len: usize,
    context: &mut IntegrationTestContext,
) {
    if spent_cell_output_len > 1 {
        // make a parent original_cell_hash for spent_cell_hash,
        // meaning that original_cell_hash won't be spendable anymore
        context.register_cell_hash(original_cell_hash, spent_cell_hash);
    } else if original_cell_output_len > 1 {
        // if previous cell had more than 1 output, then more likely it can be spent
        // again as long as have enough balance,
        // thus we reference both transactions to themselves
        context.register_cell_hash(original_cell_hash, original_cell_hash);
        context.register_cell_hash(spent_cell_hash, spent_cell_hash);
    }
}

pub async fn send_cell(
    from: &TestNode,
    to: &TestNode,
    cell: Cell,
    amount: u64,
) -> Result<Option<CellHash>> {
    if let Some(Response::GenerateTxAck(ack)) =
        client::oneshot(from.address, create_transfer_request(&from, &to, amount, cell)).await?
    {
        sleep(Duration::from_secs(2));
        return Ok(ack.cell_hash);
    } else {
        Ok(None)
    }
}

pub async fn get_cell(
    min_amount: u64,
    context: &mut IntegrationTestContext,
    node: &TestNode,
) -> Result<Option<Cell>> {
    let cell_hashes = context.get_latest_cells_of(get_cell_hashes(node.address).await?);

    get_cell_with_min_amount(min_amount, node, &cell_hashes).await
}

pub async fn get_not_spendable_cell(
    min_amount: u64,
    context: &mut IntegrationTestContext,
    node: &TestNode,
) -> Result<Option<Cell>> {
    let mut cell_hashes = get_cell_hashes(node.address).await?;
    let spendable_cell_hashes = context.get_latest_cells_of(cell_hashes.iter().cloned().collect());
    cell_hashes.retain(|cell_hash| !spendable_cell_hashes.contains(cell_hash)); // exclude all spendable transactions

    get_cell_with_min_amount(
        min_amount,
        node,
        &cell_hashes.iter().cloned().collect::<HashSet<CellHash>>(),
    )
    .await
}

pub async fn get_cell_with_min_amount(
    min_amount: u64,
    node: &TestNode,
    cell_hashes: &HashSet<CellHash>,
) -> Result<Option<Cell>> {
    get_cell_in_amount_range(min_amount, u64::MAX, node, cell_hashes).await
}

pub async fn get_cell_in_amount_range(
    min_amount: u64,
    max_amount: u64,
    node: &TestNode,
    cell_hashes: &HashSet<CellHash>,
) -> Result<Option<Cell>> {
    for cell_hash in cell_hashes {
        if let Ok(cell_option) = get_cell_from_hash(cell_hash.clone(), node.address).await {
            if cell_option.is_some() {
                let cell = cell_option.unwrap();
                let balance = get_outputs_capacity_of_owner(&cell, &node);
                if balance > min_amount && balance < max_amount {
                    // return the first match transaction
                    return Ok(Some(cell));
                }
            }
        }
    }
    Ok(None)
}

pub fn get_outputs_capacity_of_owner(cell: &Cell, owner: &TestNode) -> u64 {
    cell.outputs_of_owner(&owner.public_key)
        .iter()
        .filter_map(|o| if o.cell_type != CellType::Stake { Some(o.capacity) } else { None })
        .sum()
}

pub async fn get_cell_from_hash(
    cell_hash: CellHash,
    node_address: SocketAddr,
) -> Result<Option<Cell>> {
    if let Some(Response::CellAck(cell_ack)) = client::oneshot(
        node_address,
        Request::GetCell(sleet::GetCell { cell_hash: cell_hash.clone() }),
    )
    .await?
    {
        if let Some(cell) = cell_ack.cell {
            return Result::Ok(Some(cell));
        }
    }
    return Ok(None);
}

pub async fn get_cell_hashes(node_address: SocketAddr) -> Result<Vec<CellHash>> {
    if let Some(Response::CellHashes(cell_hashes)) =
        client::oneshot(node_address, Request::GetCellHashes).await?
    {
        let mut cell_hashes_mut = cell_hashes.ids.iter().cloned().collect::<Vec<CellHash>>();
        cell_hashes_mut.shuffle(&mut thread_rng()); // to avoid getting the same tx hash
        Result::Ok(cell_hashes_mut)
    } else {
        Result::Ok(vec![])
    }
}

pub async fn check_node_status(node_address: SocketAddr) -> Result<Option<Status>> {
    match client::oneshot(node_address, Request::CheckStatus).await {
        Ok(r) => {
            if let Some(Response::Status(status)) = r {
                Result::Ok(Some(status))
            } else {
                Result::Ok(None)
            }
        }
        _ => Result::Ok(None),
    }
}

pub fn create_transfer_request(
    from: &TestNode,
    to: &TestNode,
    spend_amount: u64,
    cell: Cell,
) -> Request {
    let transfer_op =
        TransferOperation::new(cell, to.public_key.clone(), from.public_key.clone(), spend_amount);
    Request::GenerateTx(sleet::GenerateTx { cell: transfer_op.transfer(&from.keypair).unwrap() })
}

pub fn run_nodes(nodes: &mut Vec<TestNode>) {
    tracing_subscriber::fmt()
        .with_level(false)
        .with_target(false)
        .without_time()
        .compact()
        .with_max_level(tracing::Level::INFO)
        .init();

    for node in nodes {
        node.start()
    }
}

pub async fn wait_until_nodes_start(nodes: &TestNodes) -> Result<()> {
    let mut live_nodes: HashSet<&PublicKeyHash> = HashSet::new();
    let mut timer = 0;
    let mut delay = 2;
    let mut timeout = 60;
    let nodes_size = nodes.nodes.len();

    while live_nodes.len() < nodes_size && timer <= timeout {
        sleep(Duration::from_secs(delay));
        timer += delay;
        // mark a node as 'live' if its bootstrapped status is true
        for node in &nodes.nodes {
            match check_node_status(node.address).await? {
                Some(s) => {
                    if s.bootstrapped {
                        live_nodes.insert(&node.public_key)
                    } else {
                        live_nodes.remove(&node.public_key)
                    }
                }
                None => live_nodes.remove(&node.public_key),
            };
        }
    }
    Ok(())
}

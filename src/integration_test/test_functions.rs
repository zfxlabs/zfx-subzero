use std::collections::HashSet;
use std::net::SocketAddr;
use std::thread::sleep;
use std::time::Duration;

use rand::seq::SliceRandom;
use rand::{thread_rng, Rng};
use tokio::time::timeout;
use tracing::debug;

use crate::alpha::block::Block;
use crate::alpha::status_handler::NodeStatus;
use crate::alpha::transfer::TransferOperation;
use crate::alpha::types::BlockHeight;
use crate::cell::inputs::Inputs;
use crate::cell::outputs::{Output, Outputs};
use crate::cell::types::{Capacity, CellHash, PublicKeyHash, FEE};
use crate::cell::{Cell, CellType};
use crate::hail::GetBlockByHeight;
use crate::ice::Status;
use crate::integration_test::test_model::{IntegrationTestContext, TestNode, TestNodes};
use crate::protocol::Response;
use crate::sleet::sleet_cell_handlers::GetAcceptedCell;
use crate::zfx_id::Id;
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

/// Spend a cell with amount and send it from one node to another.
///
/// Returns a hash of the spent cell, or None if request wasn't successful
/// or transfer was not accepted.
pub async fn spend_cell(
    from: &TestNode,
    to: &TestNode,
    cell: Cell,
    amount: u64,
) -> Result<Option<CellHash>> {
    let cell_hash = cell.hash();
    debug!("Sending a cell {}:{}, from = {}, to: {}", hex::encode(cell_hash), cell, from.address_as_str, to.address_as_str);

    if let Ok(Ok(Some(Response::GenerateTxAck(ack)))) = timeout(
        Duration::from_secs(5),
        client::oneshot_tcp(from.address, create_transfer_request(&from, &to, amount, cell)),
    )
    .await
    {
        Ok(ack.cell_hash)
    } else {
        debug!("No confirmation for the cell {} has been received", hex::encode(cell_hash));
        Ok(None)
    }
}

/// Spend a cell with amount and send it from one node to another.
/// If cell with hash doesn't exist, it will panic.
///
/// Returns a hash of the spent cell, or None if request wasn't successful
/// or transfer was not accepted.
pub async fn spend_cell_from_hash(
    from: &TestNode,
    to: &TestNode,
    cell_hash: CellHash,
    amount: u64,
) -> Result<Option<CellHash>> {
    if let Some(cell) = get_cell_from_hash(cell_hash, from.address).await? {
        Ok(spend_cell(from, to, cell, amount).await?)
    } else {
        panic!("cell doesn't exist: {}", hex::encode(&cell_hash));
    }
}

/// Spend any cell from a list of spendable cells with indicated amount
/// and send it from one node to another.
/// Returns an updated list of spendable cell hashes with new balance
pub async fn spend_from(
    from: &TestNode,
    to: &TestNode,
    amount: Capacity,
    spendable_cell_hashes: Vec<(CellHash, Capacity)>,
) -> Result<Vec<(CellHash, Capacity)>> {
    let total_to_spend = amount + FEE;
    let mut updated_spendable_cell_hashes = spendable_cell_hashes.clone();
    if let Some((cell_hash, capacity)) =
        spendable_cell_hashes.iter().find(|(_, c)| *c > total_to_spend)
    {
        let spent_cell_hash = spend_cell_from_hash(from, to, *cell_hash, amount).await?.unwrap();
        debug!(
            "Cell has been sent {:?} with amount {}, from = {}. Returned new cell: {:?}\n",
            hex::encode(cell_hash),
            amount,
            from.address_as_str,
            hex::encode(spent_cell_hash),
        );

        let new_capacity = capacity - total_to_spend;
        updated_spendable_cell_hashes.retain(|(h, _)| h != cell_hash);
        updated_spendable_cell_hashes.push((spent_cell_hash, new_capacity));
    }
    Ok(updated_spendable_cell_hashes)
}

/// Spend many random cells from one node and send them to another.
///
/// `Iteration` indicates number of transfers and `delay` - is a delay between transfers of cells.
///
/// Returns and list of spent cells and a list of spendable-cells with updated balance.
pub async fn spend_many(
    from: &TestNode,
    to: &TestNode,
    amount: Capacity,
    iterations: usize,
    delay: Duration,
) -> Result<(Vec<CellHash>, Vec<(CellHash, Capacity)>)> {
    spend_many_from_cell_hashes(
        from,
        to,
        amount,
        iterations,
        delay,
        get_cell_hashes_with_max_capacity(from).await,
    )
    .await
}

/// Spend many random cells from the indicated list of initial spendable cells
/// and send them from one node to another.
///
/// `Iteration` indicates number of transfers and `delay` - is a delay between transfers of cells.
///
/// Returns and list of spent cells and a list of spendable-cells with updated balance.
pub async fn spend_many_from_cell_hashes(
    from: &TestNode,
    to: &TestNode,
    amount: Capacity,
    iterations: usize,
    delay: Duration,
    initial_cell_hashes: Vec<(CellHash, Capacity)>,
) -> Result<(Vec<CellHash>, Vec<(CellHash, Capacity)>)> {
    let mut cells_hashes = initial_cell_hashes;
    let mut accepted_cell_hashes = vec![];

    for _ in 0..iterations {
        sleep(delay);
        let updated_cells_hashes =
            spend_from(from, to, amount, cells_hashes.clone()).await?.clone();
        // extract the recently spent cell
        let spent_cell_hash =
            updated_cells_hashes.iter().find(|c| !cells_hashes.contains(c)).unwrap().0;
        cells_hashes = updated_cells_hashes;
        accepted_cell_hashes.push(spent_cell_hash);
    }

    Ok((accepted_cell_hashes, cells_hashes))
}

/// Attempt to spend many accepted cells and send from one node to another.
/// This function is useful in a stress testing when you need to mix valid transfers with invalid,
/// as every attempt of cell transfer here will fail.
///
/// `Iteration` indicates number of transfers and `delay` - is a delay between transfers of cells.
pub async fn spend_many_from_accepted_cells(
    from: &TestNode,
    to: &TestNode,
    iterations: usize,
    delay: Duration,
) -> Result<()> {
    for i in 1..iterations {
        sleep(delay);

        let mut accepted_cell_hashes = get_accepted_cell_hashes(from.address)
            .await?
            .iter()
            .cloned()
            .collect::<Vec<CellHash>>();
        accepted_cell_hashes.shuffle(&mut rand::thread_rng()); // to avoid getting the same tx hash

        if let Some(cell_hash) = accepted_cell_hashes.first() {
            debug!("Attempting to spend accepted cell {}", hex::encode(cell_hash));
            if let Some(cell) = get_cell_from_hash(*cell_hash, from.address).await? {
                let spendable_amount = cell
                    .outputs_of_owner(&from.public_key)
                    .iter()
                    .map(|o| o.capacity)
                    .sum::<Capacity>();
                if spendable_amount > i as Capacity + FEE {
                    if let Some(spent_cell_hash) =
                        spend_cell(&from, &to, cell, i as Capacity).await?
                    {
                        assert!(get_cell_from_hash(spent_cell_hash.clone(), from.address)
                            .await?
                            .is_none());
                    }
                }
            }
        }
    }

    Ok(())
}

/// Attempt to spend many invalid cells and send from one node to another.
/// This function is useful in a stress testing when you need to mix valid transfers with invalid,
/// as every attempt of cell transfer here will fail.
///
/// `Iteration` indicates number of transfers and `delay` - is a delay between transfers of cells.
pub async fn spend_many_from_invalid_cells(
    from: &TestNode,
    to: &TestNode,
    iterations: usize,
    delay: Duration,
) -> Result<()> {
    for i in 1..iterations {
        sleep(delay);

        if let Some(cell_hash) = get_cell_hashes(from.address).await?.first() {
            if let Some(cell) = get_cell_from_hash(*cell_hash, from.address).await? {
                if let Some(input) = cell.inputs().iter().cloned().last() {
                    let mut inputs = HashSet::new();
                    inputs.insert(input);
                    let new_cell = Cell::new(
                        Inputs { inputs },
                        Outputs {
                            outputs: vec![Output {
                                capacity: 1000 as Capacity,
                                cell_type: CellType::Transfer,
                                data: vec![],
                                lock: from.public_key.clone(),
                            }],
                        },
                    );

                    debug!("Try to spend an invalid cell");
                    assert!(spend_cell(&from, &to, new_cell, i as Capacity).await?.is_none());
                }
            }
        }
    }

    Ok(())
}

/// Get outputs belonging to the indicated `owner`
pub async fn get_cell_outputs_of_node(
    owner: &TestNode,
    context: &mut IntegrationTestContext,
) -> Result<Vec<Output>> {
    let mut outputs: Vec<Output> = vec![];
    for cell_hash in context.get_latest_cells_of(get_cell_hashes(owner.address).await?) {
        if let Ok(cell_option) = get_cell_from_hash(cell_hash.clone(), owner.address).await {
            if cell_option.is_some() {
                cell_option
                    .unwrap()
                    .outputs_of_owner(&owner.public_key)
                    .iter()
                    .cloned()
                    .filter(|o| o.cell_type != CellType::Stake)
                    .for_each(|o| {
                        outputs.push(o.clone());
                    });
            }
        }
    }
    return Ok(outputs);
}

/// Find and get a cell which has min spendable amount
pub async fn get_cell(
    min_amount: u64,
    context: &IntegrationTestContext,
    node: &TestNode,
) -> Result<Option<Cell>> {
    let cell_hashes = context.get_latest_cells_of(get_cell_hashes(node.address).await?);

    get_cell_with_min_amount(min_amount, node, &cell_hashes).await
}

/// Finds and get a non-spendable cell which has min spendable amount.
/// The test context will filter out those cells which are spendable.
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

/// Retrieve outputs from the cell belonging to the owner and return the total balance
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
    debug!("Request to get cell from hash {:?}, from = {}", hex::encode(cell_hash), node_address);

    if let Some(Response::CellAck(cell_ack)) = filtered_request_with_timeout(
        node_address,
        Request::GetCell(sleet::GetCell { cell_hash: cell_hash.clone() }),
        |r| {
            if let Response::CellAck(cell_ack) = r {
                cell_ack.cell.is_some()
            } else {
                false
            }
        },
    )
    .await
    {
        if let Some(cell) = cell_ack.cell {
            return Result::Ok(Some(cell));
        }
    }

    return Result::Ok(None);
}

/// Get all accepted cell hashes from the node
pub async fn get_cell_hashes(node_address: SocketAddr) -> Result<Vec<CellHash>> {
    debug!("Requesting cell hashes from = {}", node_address);
    if let Some(Response::CellHashes(cell_hashes)) =
        request_with_timeout(node_address, Request::GetCellHashes).await
    {
        let mut cell_hashes_mut = cell_hashes.ids.iter().cloned().collect::<Vec<CellHash>>();
        cell_hashes_mut.shuffle(&mut thread_rng()); // to avoid getting the same tx hash
        Result::Ok(cell_hashes_mut)
    } else {
        Result::Ok(vec![])
    }
}

pub async fn get_accepted_cell_hashes(node_address: SocketAddr) -> Result<Vec<CellHash>> {
    debug!("Requesting accepted cell hashes from = {}", node_address);
    if let Some(Response::AcceptedCellHashes(cell_hashes)) =
        request_with_timeout(node_address, Request::GetAcceptedCellHashes).await
    {
        Result::Ok(cell_hashes.ids)
    } else {
        Result::Ok(vec![])
    }
}

pub async fn get_accepted_cell_from_hash(
    cell_hash: CellHash,
    node_address: SocketAddr,
) -> Result<Option<Cell>> {
    debug!(
        "Request to get accepted cell from hash {:?}, from = {}",
        hex::encode(cell_hash),
        node_address
    );

    if let Some(Response::AcceptedCellAck(cell_ack)) = filtered_request_with_timeout(
        node_address,
        Request::GetAcceptedCell(GetAcceptedCell { cell_hash: cell_hash.clone() }),
        |r| {
            if let Response::AcceptedCellAck(cell_ack) = r {
                cell_ack.cell.is_some()
            } else {
                false
            }
        },
    )
    .await
    {
        if let Some(cell) = cell_ack.cell {
            return Result::Ok(Some(cell));
        }
    }

    return Result::Ok(None);
}

/// Get block by height
pub async fn get_block(node_address: SocketAddr, height: BlockHeight) -> Result<Option<Block>> {
    debug!("Request to get block with height {:?}, from = {}", height, node_address);

    if let Some(Response::BlockAck(block)) = filtered_request_with_timeout(
        node_address,
        Request::GetBlockByHeight(GetBlockByHeight { block_height: height }),
        |r| {
            if let Response::BlockAck(b) = r {
                b.block.is_some()
            } else {
                false
            }
        },
    )
    .await
    {
        return Result::Ok(block.block);
    }
    return Result::Ok(None);
}

/// Get all cell hashes of the node with balances
pub async fn get_cell_hashes_with_max_capacity(node: &TestNode) -> Vec<(CellHash, Capacity)> {
    let mut initial_cells_hashes: Vec<(CellHash, Capacity)> = vec![];
    for cell_hash in get_cell_hashes(node.address).await.unwrap() {
        let cell = get_cell_from_hash(cell_hash, node.address).await.unwrap();
        let max_capacity = cell
            .unwrap()
            .outputs_of_owner(&node.public_key)
            .iter()
            .filter(|o| o.cell_type != CellType::Stake)
            .map(|o| o.capacity)
            .sum::<u64>();
        if max_capacity > 0 {
            initial_cells_hashes.push((cell_hash, max_capacity));
        }
    }
    initial_cells_hashes
}

async fn get_cell_with_min_amount(
    min_amount: u64,
    node: &TestNode,
    cell_hashes: &HashSet<CellHash>,
) -> Result<Option<Cell>> {
    get_cell_in_amount_range(min_amount, u64::MAX, node, cell_hashes).await
}

async fn get_cell_in_amount_range(
    min_amount: u64,
    max_amount: u64,
    node: &TestNode,
    cell_hashes: &HashSet<CellHash>,
) -> Result<Option<Cell>> {
    for cell_hash in cell_hashes {
        if let Ok(cell_option) = get_cell_from_hash(cell_hash.clone(), node.address).await {
            if cell_option.is_some() {
                let cell = cell_option.unwrap();
                let balance = get_outputs_capacity_of_owner_including_fee(&node, &cell);

                if balance > min_amount && balance < max_amount {
                    // return the first match transaction
                    return Ok(Some(cell));
                }
            }
        }
    }
    Ok(None)
}

fn get_outputs_capacity_of_owner_including_fee(node: &&TestNode, cell: &Cell) -> u64 {
    let balance = get_outputs_capacity_of_owner(&cell, &node);
    return if balance > FEE { balance - FEE } else { 0 };
}

async fn request_with_timeout(node_address: SocketAddr, request: Request) -> Option<Response> {
    filtered_request_with_timeout_and_attempts(
        node_address,
        request,
        Duration::from_millis(10),
        1000,
        |_| true,
    )
    .await
}

async fn request_with_timeout_and_attempts(
    node_address: SocketAddr,
    request: Request,
    duration: Duration,
    attempts: usize,
) -> Option<Response> {
    filtered_request_with_timeout_and_attempts(node_address, request, duration, attempts, |_| true)
        .await
}

async fn filtered_request_with_timeout<P>(
    node_address: SocketAddr,
    request: Request,
    predicate: P,
) -> Option<Response>
where
    P: Fn(Response) -> bool,
{
    filtered_request_with_timeout_and_attempts(
        node_address,
        request,
        Duration::from_millis(10),
        1000,
        predicate,
    )
    .await
}

async fn filtered_request_with_timeout_and_attempts<P>(
    node_address: SocketAddr,
    request: Request,
    duration: Duration,
    attempts: usize,
    predicate: P,
) -> Option<Response>
where
    P: Fn(Response) -> bool,
{
    let mut result: Option<Response> = None;
    let mut updated_attempts = attempts;
    while updated_attempts > 0 {
        if let Ok(Ok(r)) =
            timeout(duration, client::oneshot_tcp(node_address, request.clone())).await
        {
            if r.is_some() && predicate(r.clone().unwrap()) {
                result = r;
                break;
            }
        }
        updated_attempts = updated_attempts - 1;
    }
    result
}

fn create_transfer_request(
    from: &TestNode,
    to: &TestNode,
    spend_amount: u64,
    cell: Cell,
) -> Request {
    let transfer_op =
        TransferOperation::new(cell, to.public_key.clone(), from.public_key.clone(), spend_amount);
    Request::GenerateTx(sleet::GenerateTx { cell: transfer_op.transfer(&from.keypair).unwrap() })
}

/// Regularly check status of the nodes until all of them are bootstrapped.
pub async fn wait_until_nodes_start(nodes: &TestNodes) -> Result<()> {
    let mut live_nodes: HashSet<&PublicKeyHash> = HashSet::new();
    let mut timer = 0;
    let timeout = 120;
    let delay = 2;
    let nodes_size = nodes.get_running_nodes().len();

    while live_nodes.len() < nodes_size && timer <= timeout {
        sleep(Duration::from_secs(delay));
        timer += delay;
        // mark a node as 'live' if its bootstrapped status is true
        for node in &nodes.get_running_nodes() {
            match get_node_status(node.address).await? {
                Some(s) => {
                    if s.bootstrapped {
                        debug!("Node {} has been bootstrapped", &node.address);
                        live_nodes.insert(&node.public_key)
                    } else {
                        live_nodes.remove(&node.public_key)
                    }
                }
                None => live_nodes.remove(&node.public_key),
            };
        }
    }
    debug!("All nodes have been started up and bootstrapped");
    Ok(())
}

pub async fn get_node_status(node_address: SocketAddr) -> Result<Option<NodeStatus>> {
    match timeout(Duration::from_secs(1), client::oneshot_tcp(node_address, Request::GetNodeStatus))
        .await
    {
        Ok(Ok(r)) => {
            if let Some(Response::NodeStatus(status)) = r {
                Result::Ok(Some(status))
            } else {
                Result::Ok(None)
            }
        }
        _ => Result::Ok(None),
    }
}

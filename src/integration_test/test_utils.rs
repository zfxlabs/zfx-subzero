use std::collections::HashSet;
use std::net::SocketAddr;
use std::thread::sleep;
use std::time::Duration;

use ed25519_dalek::Keypair;
use rand::seq::SliceRandom;
use rand::thread_rng;

use crate::chain::alpha::{Transaction, TransferTx, Tx, TxHash, FEE};
use crate::channel::Error;
use crate::integration_test::test_model::{IntegrationTestContext, TestNode};
use crate::protocol::Response;
use crate::Result;
use crate::{client, sleet, Request};

/// Register tx and it's parent to identify later which tx can be spent
pub fn register_tx_in_test_context(
    original_tx_hash: TxHash,
    spent_tx_hash: TxHash,
    spent_tx_output_len: usize,
    original_tx_output_len: usize,
    context: &mut IntegrationTestContext,
) {
    if spent_tx_output_len > 1 {
        // make a parent original_tx_hash for spent_tx_hash,
        // meaning that original_tx_hash won't be spendable anymore
        context.register_tx_hash(original_tx_hash, spent_tx_hash);
    } else if original_tx_output_len > 1 {
        // if previous tx had more than 1 output, then more likely it can be spent
        // again as long as have enough balance,
        // thus we reference both transactions to themselves
        context.register_tx_hash(original_tx_hash, original_tx_hash);
        context.register_tx_hash(spent_tx_hash, spent_tx_hash);
    }
}

pub async fn send_tx(
    from: &TestNode,
    to: &TestNode,
    tx_hash: TxHash,
    tx: Tx,
    amount: u64,
) -> Result<Option<TxHash>> {
    if let Some(Response::GenerateTxAck(ack)) =
        client::oneshot(from.address, create_transfer_request(&from, &to, amount, tx_hash, tx))
            .await?
    {
        sleep(Duration::from_secs(2));
        return Ok(ack.tx_hash);
    } else {
        Ok(None)
    }
}

pub async fn get_tx(
    min_amount: u64,
    context: &mut IntegrationTestContext,
    node_address: SocketAddr,
) -> Result<Option<(TxHash, Tx)>> {
    let tx_hashes = context.get_latest_txs_of(get_tx_hashes(node_address).await?);

    get_tx_with_min_amount(min_amount, node_address, &tx_hashes).await
}

pub async fn get_not_spendable_tx(
    min_amount: u64,
    context: &mut IntegrationTestContext,
    node_address: SocketAddr,
) -> Result<Option<(TxHash, Tx)>> {
    let mut tx_hashes = get_tx_hashes(node_address).await?;
    let spendable_tx_hashes = context.get_latest_txs_of(tx_hashes.iter().cloned().collect());
    tx_hashes.retain(|tx_hash| !spendable_tx_hashes.contains(tx_hash)); // exclude all spendable transactions

    get_tx_with_min_amount(min_amount, node_address, &tx_hashes.iter().cloned().collect::<HashSet<TxHash>>())
        .await
}

pub async fn get_tx_with_min_amount(
    min_amount: u64,
    node_address: SocketAddr,
    tx_hashes: &HashSet<TxHash>,
) -> Result<Option<(TxHash, Tx)>> {
    for tx_hash in tx_hashes {
        if let Ok(tx_option) = get_tx_from_hash(tx_hash.clone(), node_address).await {
            if tx_option.is_some() {
                let tx = tx_option.unwrap();
                if tx.sum() > min_amount {
                    // return the first match transaction
                    return Ok(Some((tx_hash.clone(), tx)));
                }
            }
        }
    }
    Ok(None)
}

pub async fn get_tx_from_hash(tx_hash: TxHash, node_address: SocketAddr) -> Result<Option<Tx>> {
    if let Some(Response::TxAck(tx_ack)) =
        client::oneshot(node_address, Request::GetTx(sleet::GetTx { tx_hash: tx_hash.clone() }))
            .await?
    {
        if let Some(tx) = tx_ack.tx {
            return Result::Ok(Some(tx.inner()));
        }
    }
    return Ok(None);
}

pub async fn get_tx_hashes(node_address: SocketAddr) -> Result<Vec<TxHash>> {
    if let Some(Response::Transactions(txs)) =
        client::oneshot(node_address, Request::GetTransactions).await?
    {
        let mut txs_mut = txs.ids.iter().cloned().collect::<Vec<TxHash>>();
        txs_mut.shuffle(&mut thread_rng()); // to avoid getting the same tx hash
        Result::Ok(txs_mut)
    } else {
        Result::Ok(vec![])
    }
}

pub fn create_transfer_request(
    from: &TestNode,
    to: &TestNode,
    spend_amount: u64,
    tx_hash: TxHash,
    tx: Tx,
) -> Request {
    Request::GenerateTx(sleet::GenerateTx {
        tx: Transaction::TransferTx(TransferTx::new(
            &from.keypair,
            tx_hash,
            tx,
            to.public_key.clone(),
            from.public_key.clone(),
            spend_amount,
        )),
    })
}

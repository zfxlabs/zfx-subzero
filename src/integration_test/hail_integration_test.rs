#[cfg(test)]
#[cfg(feature = "hail_test")]
mod hail_test {
    use crate::alpha::transfer::TransferOperation;
    use crate::cell::types::CellHash;
    use crate::client;
    use crate::integration_test::test_functions::*;
    use crate::integration_test::test_model::{IntegrationTestContext, TestNode, TestNodes};
    use crate::protocol::{Request, Response};
    use crate::sleet;
    use crate::Result;

    use std::time::Duration;
    use tracing::info;

    // We know that this output has 2000 to spend.
    // 9 transactions will be accepted from the 19 issued.
    const INITIAL_HASH: &str = "b5fba12b605e166987f031c300e33969e07e295285a3744692f326535fba555e";
    const ITERATIONS: u64 = 19;

    #[actix_rt::test]
    async fn run_hail_test() -> Result<()> {
        let mut context = IntegrationTestContext::new();
        let mut nodes = TestNodes::new();

        nodes.start_all();
        wait_until_nodes_start(&nodes).await?;

        let node = nodes.get_node(0).unwrap();
        let mut cell_hash = starting_hash();
        for i in 0..ITERATIONS {
            cell_hash = spend_cell(&node, cell_hash, 1 + i).await?;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Ok(())
    }

    fn starting_hash() -> CellHash {
        let cell_hash_vec = hex::decode(INITIAL_HASH).unwrap();
        let mut cell_hash_bytes = [0u8; 32];
        cell_hash_bytes.copy_from_slice(&cell_hash_vec[..32]);
        cell_hash_bytes
    }

    /// Spend the specified cell and return its output
    async fn spend_cell(node: &TestNode, cell_hash: CellHash, amount: u64) -> Result<CellHash> {
        if let Some(Response::CellAck(sleet::CellAck { cell: Some(cell_in) })) = client::oneshot(
            node.address,
            Request::GetCell(sleet::GetCell { cell_hash: cell_hash.clone() }),
        )
        .await?
        {
            // info!("spendable:\n{}\n", cell_in.clone());
            let transfer_op = TransferOperation::new(
                cell_in.clone(),
                node.public_key.clone(),
                node.public_key.clone(),
                amount + 1,
            );
            let transfer_tx = transfer_op.transfer(&node.keypair).unwrap();
            let new_cell_hash = transfer_tx.hash();
            match client::oneshot(
                node.address,
                Request::GenerateTx(sleet::GenerateTx { cell: transfer_tx.clone() }),
            )
            .await?
            {
                Some(Response::GenerateTxAck(sleet::GenerateTxAck { cell_hash: Some(h) })) => {
                    assert_eq!(h, new_cell_hash)
                }
                other => panic!("Unexpected: {:?}", other),
            }
            // info!("sent tx:\n{:?}\n", tx.clone());
            // info!("new cell_hash: {}", hex::encode(&new_cell_hash));
            Ok(new_cell_hash)
        } else {
            panic!("cell doesn't exist: {}", hex::encode(&cell_hash));
        }
    }
}

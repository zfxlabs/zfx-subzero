#[cfg(test)]
mod integration_test {
    use crate::client;
    use crate::chain::alpha::{FEE, Tx, TxHash};
    use crate::integration_test::test_model::{IntegrationTestContext, TestNode, TestNodes};
    use crate::integration_test::test_utils::*;
    use crate::Result;

    #[actix_rt::test]
    async fn run_integration_test_suite() -> Result<()> {
        let mut context = IntegrationTestContext::new();

        test_send_tx(&mut context).await?;
        test_send_tx(&mut context).await?;
        test_send_tx(&mut context).await?;
        test_send_tx(&mut context).await?;

        test_send_same_tx_twice(&mut context).await?;
        test_send_same_tx_twice(&mut context).await?;
        test_multi_spend_same_tx(&mut context).await?;

        Result::Ok(())
    }

    async fn test_send_tx(context: &mut IntegrationTestContext) -> Result<()> {
        let nodes = TestNodes::new();
        let node_0 = nodes.get_node(0).unwrap();
        let node_1 = nodes.get_node(1).unwrap();
        let spend_amount = 10 + context.test_run_counter as u64; // send diff amount to avoid duplicated txs

        let (tx_hash, tx) = get_tx(spend_amount, context, node_0.address).await?.unwrap();
        let previous_balance = tx.sum();
        let previous_len = tx.outputs.len();

        let spent_tx_hash = send_tx(&node_0, &node_1, tx_hash, tx, spend_amount).await?;
        assert!(spent_tx_hash.is_some());

        // validate tx output and input
        let spent_tx = get_tx_from_hash(spent_tx_hash.unwrap().clone(), node_0.address).await?;

        assert_tx(
            spent_tx.unwrap(),
            spent_tx_hash.unwrap(),
            tx_hash,
            previous_len,
            previous_balance,
            spend_amount,
            node_0,
            node_1,
            context,
        );

        context.count_test_run();

        Result::Ok(())
    }

    async fn test_multi_spend_same_tx(context: &mut IntegrationTestContext) -> Result<()> {
        let nodes = TestNodes::new();
        let node_0 = nodes.get_node(0).unwrap();
        let node_1 = nodes.get_node(1).unwrap();
        let spend_amount = 40;

        let (tx_hash, tx) = get_tx(spend_amount, context, node_0.address).await?.unwrap();
        let previous_output_len = tx.outputs.len();

        let spent_tx_hash = send_tx(&node_0, &node_1, tx_hash, tx, spend_amount).await?;
        assert!(spent_tx_hash.is_some());
        let spent_tx = get_tx_from_hash(spent_tx_hash.unwrap().clone(), node_0.address).await?;
        register_tx_in_test_context(tx_hash, spent_tx_hash.unwrap(), spent_tx.unwrap().outputs.len(), previous_output_len, context);

        // try to send different amount many times for the same origin tx
        for i in 0..3 {
            let (tx_hash, tx) = get_not_spendable_tx(spend_amount + i, context, node_0.address).await?.unwrap();

            // change amount to avoid duplicated tx
            let spent_tx_hash = send_tx(&node_0, &node_1, tx_hash, tx, spend_amount + i).await?;

            if spent_tx_hash.is_some() {
                let spent_tx = get_tx_from_hash(spent_tx_hash.unwrap().clone(), node_0.address).await?;
                assert!(spent_tx.is_none())
            }
        }

        context.count_test_run();
        Result::Ok(())
    }

    async fn test_send_same_tx_twice(context: &mut IntegrationTestContext) -> Result<()> {
        let nodes = TestNodes::new();
        let node_0 = nodes.get_node(0).unwrap();
        let node_1 = nodes.get_node(1).unwrap();
        let spend_amount: u64 = 30;

        let (tx_hash, tx) = get_tx(2 * spend_amount, context, node_0.address).await?.unwrap();
        let previous_output_len = tx.outputs.len();

        let spent_tx_hash = send_tx(&node_0, &node_1, tx_hash, tx, spend_amount).await?.unwrap();
        let spent_tx = get_tx_from_hash(spent_tx_hash.clone(), node_0.address).await?.unwrap();
        assert_eq!(spend_amount, spent_tx.outputs[0].value); // check if first transfer was successful

        register_tx_in_test_context(
            tx_hash,
            spent_tx_hash,
            spent_tx.outputs.len(),
            previous_output_len,
            context,
        );

        let same_tx = get_tx_from_hash(tx_hash.clone(), node_0.address).await?.unwrap();
        let duplicated_tx_hash = send_tx(&node_0, &node_1, tx_hash, same_tx, spend_amount).await?;
        assert!(duplicated_tx_hash.is_none()); // check the duplicated tx was rejected

        context.count_test_run();
        Result::Ok(())
    }

    fn assert_tx(
        spent_tx: Tx,
        spent_tx_hash: TxHash,
        tx_hash: TxHash,
        previous_len: usize,
        previous_balance: u64,
        spend_amount: u64,
        node_0: &TestNode,
        node_1: &TestNode,
        context: &mut IntegrationTestContext,
    ) {
        if spent_tx.outputs.len() > 1 {
            assert_eq!(2, spent_tx.outputs.len(), "Tx must have spent and remaining outputs");
            assert_eq!(
                previous_balance - FEE - spend_amount,
                spent_tx.outputs[1].value,
                "Invalid balance of the remaining output"
            );
            assert_eq!(
                node_0.public_key, spent_tx.outputs[1].owner_hash,
                "Invalid owner of the remaining output"
            );
        } else {
            assert_eq!(1, spent_tx.outputs.len(), "Tx must have only spent output");
        }
        assert_eq!(
            node_1.public_key, spent_tx.outputs[0].owner_hash,
            "Invalid owner of the spent output"
        );
        assert_eq!(spend_amount, spent_tx.outputs[0].value, "Invalid balance of the spent output");

        assert_eq!(previous_len, spent_tx.inputs.len());
        for i in 0..previous_len {
            assert_eq!(
                i as u8, spent_tx.inputs[i].i,
                "Tx input index must be always 0 as we have a single output to spend"
            );
            assert_eq!(
                tx_hash, spent_tx.inputs[i].source,
                "Invalid source (parent) of tx from which we consume amount"
            );
            assert_eq!(
                node_0.keypair.public.as_bytes(),
                spent_tx.inputs[i].owner.as_bytes(),
                "Invalid tx owner in the input"
            );
        }

        register_tx_in_test_context(
            tx_hash,
            spent_tx_hash,
            spent_tx.outputs.len(),
            previous_len,
            context,
        );
    }
}

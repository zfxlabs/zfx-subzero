#[cfg(test)]
mod integration_test {
    use zerocopy::AsBytes;
    use crate::chain::alpha::{Transaction, Tx, TxHash, FEE};
    use crate::client;
    use crate::integration_test::test_model::{IntegrationTestContext, TestNode, TestNodes};
    use crate::integration_test::test_utils::*;
    use crate::Result;

    #[actix_rt::test]
    async fn run_integration_test_suite() -> Result<()> {
        let mut context = IntegrationTestContext::new();

        test_send_tx_with_invalid_hash(&mut context).await?;

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

        let result = send_tx_and_get_result(node_0, node_1, spend_amount, context).await?;

        assert_tx(
            result.spent_tx,
            result.spent_tx_hash,
            result.original_tx_hash,
            result.original_tx_output_len,
            result.original_tx_balance,
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

        send_tx_and_get_result(node_0, node_1, spend_amount, context).await?;

        // try to send different amount many times for the same origin tx
        for i in 0..3 {
            let (tx_hash, tx) =
                get_not_spendable_tx(spend_amount + i, context, node_0.address).await?.unwrap();

            // change amount to avoid duplicated tx
            let spent_tx_hash = send_tx(&node_0, &node_1, tx_hash, tx, spend_amount + i).await?;

            if spent_tx_hash.is_some() {
                let spent_tx =
                    get_tx_from_hash(spent_tx_hash.unwrap().clone(), node_0.address).await?;
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

        let result = send_tx_and_get_result(node_0, node_1, spend_amount, context).await?;

        let same_tx = get_tx_from_hash(result.original_tx_hash, node_0.address).await?.unwrap();
        let duplicated_tx_hash = send_tx(&node_0, &node_1, result.original_tx_hash, same_tx, spend_amount).await?;
        assert!(duplicated_tx_hash.is_none()); // check the duplicated tx was rejected

        context.count_test_run();
        Result::Ok(())
    }

    async fn test_send_tx_with_diff_hash(context: &mut IntegrationTestContext) -> Result<()> {
        let nodes = TestNodes::new();
        let node_0 = nodes.get_node(0).unwrap();
        let node_1 = nodes.get_node(1).unwrap();
        let spend_amount1 = 340 as u64;
        let spend_amount2 = 685 as u64;

        send_tx_and_get_result(node_0, node_1, spend_amount1, context);

        let (tx_hash2, tx2) = get_tx_in_range(10, 600, context, node_0.address).await?.unwrap();
        let (tx_hash3, tx3) = get_tx(spend_amount2 + FEE, context, node_0.address).await?.unwrap();

        // use tx hash of diff transaction with spendable amount less that for tx3
        let spent_tx_hash = send_tx(&node_0, &node_1, tx_hash2, tx3, spend_amount2).await?;
        assert!(spent_tx_hash.is_none());

        context.count_test_run();

        Result::Ok(())
    }

    async fn test_send_tx_with_invalid_hash(context: &mut IntegrationTestContext) -> Result<()> {
        let nodes = TestNodes::new();
        let node_0 = nodes.get_node(0).unwrap();
        let node_1 = nodes.get_node(1).unwrap();
        let spend_amount = 5 as u64;

        let (tx_hash, tx) = get_tx(spend_amount, context, node_0.address).await?.unwrap();

        // use non-existent tx hash
        let mut non_existing_tx_hash = tx_hash.clone();
        non_existing_tx_hash[0] += 1 as u8;

        let spent_tx_hash = send_tx(&node_0, &node_1, TxHash::from(non_existing_tx_hash), tx, spend_amount).await?;
        assert!(spent_tx_hash.is_none());

        context.count_test_run();

        Result::Ok(())
    }

    async fn send_tx_and_get_result(
        from: &TestNode,
        to: &TestNode,
        amount: u64,
        context: &mut IntegrationTestContext
    ) -> Result<SendTxResult> {
        let (tx_hash, tx) = get_tx(amount, context, from.address).await?.unwrap();
        let previous_output_len = tx.outputs.len();
        let previous_balance = tx.sum();

        let spent_tx_hash = send_tx(from, to, tx_hash, tx, amount).await?;
        assert!(spent_tx_hash.is_some());

        let spent_tx = get_tx_from_hash(spent_tx_hash.unwrap().clone(), from.address).await?;
        assert!(spent_tx.is_some());
        assert_eq!(amount, spent_tx.as_ref().unwrap().outputs[0].value);  // check if transfer was successful
        let spent_tx_output_len = spent_tx.as_ref().unwrap().outputs.len();

        register_tx_in_test_context(
            tx_hash,
            spent_tx_hash.unwrap(),
            spent_tx_output_len,
            previous_output_len,
            context,
        );

        Ok(SendTxResult {
            original_tx_balance: previous_balance,
            original_tx_output_len: previous_output_len,
            original_tx_hash: tx_hash,
            spent_tx: spent_tx.unwrap(),
            spent_tx_hash: spent_tx_hash.unwrap()
        })
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

    struct SendTxResult {
        original_tx_balance: u64,
        original_tx_output_len : usize,
        original_tx_hash : TxHash,
        spent_tx : Tx,
        spent_tx_hash : TxHash,
    }
}

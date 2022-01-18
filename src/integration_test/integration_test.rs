#[cfg(test)]
mod integration_test {
    use crate::chain::alpha::{Input, StakeTx, Transaction, Tx, TxHash, FEE};
    use crate::integration_test::test_model::{IntegrationTestContext, TestNode, TestNodes};
    use crate::integration_test::test_utils::*;
    use crate::zfx_id::Id;
    use crate::Result;

    #[actix_rt::test]
    async fn run_integration_test_suite() -> Result<()> {
        let mut context = IntegrationTestContext::new();

        test_send_tx(&mut context).await?;
        test_send_tx(&mut context).await?;
        test_send_tx(&mut context).await?;
        test_send_tx(&mut context).await?;

        test_send_tx_with_invalid_hash(&mut context).await?;
        test_send_tx_to_non_existing_recipient(&mut context).await?;

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
            let tx =
                get_not_spendable_tx(spend_amount + i, context, node_0.address).await?.unwrap();

            // change amount to avoid duplicated tx
            let spent_tx_hash = send_tx(&node_0, &node_1, tx, spend_amount + i).await?;

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
        let duplicated_tx_hash = send_tx(&node_0, &node_1, same_tx, spend_amount).await?;
        assert!(duplicated_tx_hash.is_none()); // check the duplicated tx was rejected

        context.count_test_run();
        Result::Ok(())
    }

    async fn test_send_tx_with_invalid_hash(context: &mut IntegrationTestContext) -> Result<()> {
        let nodes = TestNodes::new();
        let node_0 = nodes.get_node(0).unwrap();
        let node_1 = nodes.get_node(1).unwrap();
        let spend_amount = 5 as u64;

        let tx = get_tx(spend_amount, context, node_0.address).await?.unwrap();
        let odd_tx = Transaction::StakeTx(StakeTx::new(
            &node_0.keypair,
            Id::generate(),
            Tx::new(tx.inputs(), tx.outputs()),
            spend_amount,
        ));

        let spent_tx_hash = send_tx(&node_0, &node_1, odd_tx, spend_amount).await?;
        assert!(spent_tx_hash.is_none());

        context.count_test_run();

        Result::Ok(())
    }

    async fn test_send_tx_to_non_existing_recipient(
        context: &mut IntegrationTestContext,
    ) -> Result<()> {
        let nodes = TestNodes::new();
        let from = nodes.get_node(0).unwrap();
        let to = nodes.get_non_existing_node();
        let spend_amount = 15 as u64;
        let tx = get_tx(spend_amount, context, from.address).await?.unwrap();

        let spent_tx_hash = send_tx(&from, &to, tx, spend_amount).await?;
        assert!(spent_tx_hash.is_none());

        context.count_test_run();

        Result::Ok(())
    }

    async fn send_tx_and_get_result(
        from: &TestNode,
        to: &TestNode,
        amount: u64,
        context: &mut IntegrationTestContext,
    ) -> Result<SendTxResult> {
        let tx = get_tx(amount, context, from.address).await?.unwrap();
        let tx_hash = tx.hash();
        let previous_output_len = tx.inner().outputs().len();
        let previous_balance = tx.inner().sum();

        let spent_tx_hash = send_tx(from, to, tx, amount).await?;
        assert!(spent_tx_hash.is_some());

        let spent_tx = get_tx_from_hash(spent_tx_hash.unwrap().clone(), from.address).await?;
        assert!(spent_tx.is_some());

        let mut spent_tx_outputs = spent_tx.as_ref().unwrap().inner().outputs();
        assert!(spent_tx_outputs.iter().find(|o| { o.value == amount}).is_some()); // check if transfer was successful

        register_tx_in_test_context(
            tx_hash,
            spent_tx_hash.unwrap(),
            spent_tx_outputs.len(),
            previous_output_len,
            context,
        );

        Ok(SendTxResult {
            original_tx_balance: previous_balance,
            original_tx_output_len: previous_output_len,
            original_tx_hash: tx_hash,
            spent_tx: spent_tx.unwrap(),
        })
    }

    fn assert_tx(
        spent_tx: Transaction,
        tx_hash: TxHash,
        previous_len: usize,
        previous_balance: u64,
        spend_amount: u64,
        from: &TestNode,
        to: &TestNode,
        context: &mut IntegrationTestContext,
    ) {
        let spent_tx_hash = spent_tx.hash();
        let spent_tx_inputs = &spent_tx.inner().inputs();
        let spent_tx_outputs = &spent_tx.inner().outputs();
        let spent_tx_len = spent_tx_outputs.len();

        // validate outputs
        if spent_tx_len > 1 {
            assert_eq!(2, spent_tx_len, "Tx must have spent and remaining outputs");

            let remaining_output = spent_tx_outputs.iter().find(|o| { o.owner_hash == from.public_key});
            assert!(
                remaining_output.is_some(),
                "The remaining output doesn't exist"
            );
            assert_eq!(
                previous_balance - FEE - spend_amount,
                remaining_output.unwrap().value,
                "Invalid balance of the remaining output"
            );
        } else {
            assert_eq!(1, spent_tx_len, "Tx must have only spent output");
        }
        let spent_output = spent_tx_outputs.iter().find(|o| { o.owner_hash == to.public_key });
        assert!(spent_output.is_some(), "The spent output doesn't exist");
        assert_eq!(spend_amount, spent_output.unwrap().value, "Invalid balance of the spent output");

        // validate inputs
        assert_eq!(previous_len, spent_tx_inputs.len());
        let mut inputs_as_vec = spent_tx_inputs.inputs.iter().cloned().collect::<Vec<Input>>();
        inputs_as_vec.sort();
        let mut i = 0;
        for input in inputs_as_vec {
            assert_eq!(
                i as u8, input.i,
                "Tx input index must be always 0 as we have a single output to spend"
            );
            assert_eq!(
                tx_hash, input.source,
                "Invalid source (parent) of tx from which we consume amount"
            );
            assert_eq!(
                from.keypair.public.as_bytes(),
                input.owner.as_bytes(),
                "Invalid tx owner in the input"
            );
            i += 1;
        }

        register_tx_in_test_context(tx_hash, spent_tx_hash, spent_tx_len, previous_len, context);
    }

    struct SendTxResult {
        original_tx_balance: u64,
        original_tx_output_len: usize,
        original_tx_hash: TxHash,
        spent_tx: Transaction,
    }
}

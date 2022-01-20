#[cfg(test)]
mod integration_test {
    use crate::alpha::coinbase::CoinbaseOperation;
    use crate::alpha::stake::StakeOperation;
    use crate::alpha::transfer::TransferOperation;
    use crate::cell::inputs::Input;
    use crate::cell::types::{CellHash, FEE};
    use crate::cell::Cell;
    use crate::integration_test::test_model::{IntegrationTestContext, TestNode, TestNodes};
    use crate::integration_test::test_utils::*;
    use crate::zfx_id::Id;
    use crate::Result;
    use std::borrow::BorrowMut;
    use std::thread::sleep;
    use std::time::Duration;

    const TRANSFER_RUN_TIMES: i32 = 5;

    #[actix_rt::test]
    async fn run_integration_test_suite() -> Result<()> {
        let mut context = IntegrationTestContext::new();
        let mut nodes = TestNodes::new();

        run_nodes(&mut nodes.nodes);

        sleep(Duration::from_secs(10));
        test_get_txs_when_quorum_not_reached_yet(&nodes, &mut context).await?;

        sleep(Duration::from_secs(40));

        for _ in 0..TRANSFER_RUN_TIMES {
            test_send_cell(&nodes, &mut context).await?;
        }

        test_send_cell_with_invalid_hash(&nodes, &mut context).await?;
        // test_send_cell_to_non_existing_recipient(&nodes, &mut context).await?;

        test_send_same_cell_twice(&nodes, &mut context).await?;
        test_send_same_cell_twice(&nodes, &mut context).await?;

        test_multi_spend_same_cell(&nodes, &mut context).await?;

        test_get_txs_from_faulty_node(&mut nodes, &mut context).await?;

        Result::Ok(())
    }

    async fn test_send_cell(nodes: &TestNodes, context: &mut IntegrationTestContext) -> Result<()> {
        let node_0 = nodes.get_node(0).unwrap();
        let node_1 = nodes.get_node(1).unwrap();
        let spend_amount = 10 + context.test_run_counter as u64; // send diff amount to avoid duplicated txs

        let result = send_cell_and_get_result(node_0, node_1, spend_amount, context).await?;

        assert_cell(
            result.spent_cell,
            result.original_cell_hash,
            result.original_cell_output_len,
            result.original_cell_balance,
            spend_amount,
            node_0,
            node_1,
            context,
        );

        context.count_test_run();

        Result::Ok(())
    }

    async fn test_multi_spend_same_cell(
        nodes: &TestNodes,
        context: &mut IntegrationTestContext,
    ) -> Result<()> {
        let from = nodes.get_node(0).unwrap();
        let to = nodes.get_node(1).unwrap();
        let spend_amount = 40;

        send_cell_and_get_result(from, to, spend_amount, context).await?;

        // try to send different amount many times for the same origin cell
        for i in 0..3 {
            let cell = get_not_spendable_cell(spend_amount + i, context, from).await?.unwrap();

            // change amount to avoid duplicated cell
            let spent_cell_hash = send_cell(&from, &to, cell, spend_amount + i).await?;

            if spent_cell_hash.is_some() {
                let spent_cell =
                    get_cell_from_hash(spent_cell_hash.unwrap().clone(), from.address).await?;
                assert!(spent_cell.is_none())
            }
        }

        context.count_test_run();
        Result::Ok(())
    }

    async fn test_send_same_cell_twice(
        nodes: &TestNodes,
        context: &mut IntegrationTestContext,
    ) -> Result<()> {
        let from = nodes.get_node(0).unwrap();
        let to = nodes.get_node(1).unwrap();
        let spend_amount: u64 = 30;

        let result = send_cell_and_get_result(from, to, spend_amount, context).await?;

        let same_cell = get_cell_from_hash(result.original_cell_hash, from.address).await?.unwrap();
        let duplicated_cell_hash = send_cell(&from, &to, same_cell, spend_amount).await?;
        assert!(duplicated_cell_hash.is_none()); // check the duplicated cell was rejected

        context.count_test_run();
        Result::Ok(())
    }

    async fn test_send_cell_with_invalid_hash(
        nodes: &TestNodes,
        context: &mut IntegrationTestContext,
    ) -> Result<()> {
        let from = nodes.get_node(0).unwrap();
        let to = nodes.get_node(1).unwrap();
        let spend_amount = 5 as u64;

        let cell = get_cell(spend_amount, context, from).await?.unwrap();
        let odd_stake_op = TransferOperation::new(
            cell.clone(),
            Id::generate().bytes(),
            from.public_key,
            spend_amount,
        );
        let odd_stake_cell = odd_stake_op.transfer(&from.keypair).unwrap();

        let spent_cell_hash = send_cell(&from, &to, odd_stake_cell, spend_amount).await?;
        assert!(spent_cell_hash.is_none());

        context.count_test_run();

        Result::Ok(())
    }

    async fn test_send_cell_to_non_existing_recipient(
        nodes: &TestNodes,
        context: &mut IntegrationTestContext,
    ) -> Result<()> {
        let from = nodes.get_node(0).unwrap();
        let to = nodes.get_non_existing_node();
        let spend_amount = 65 as u64;
        let cell = get_cell(spend_amount, context, from).await?.unwrap();

        let spent_cell_hash = send_cell(&from, &to, cell, spend_amount).await?;
        assert!(spent_cell_hash.is_none());

        context.count_test_run();

        Result::Ok(())
    }

    async fn test_get_txs_when_quorum_not_reached_yet(
        nodes: &TestNodes,
        context: &mut IntegrationTestContext,
    ) -> Result<()> {
        let from = nodes.get_node(0).unwrap();

        let tx_hashes = get_cell_hashes(from.address).await?;
        assert!(tx_hashes.is_empty());

        context.count_test_run();

        Result::Ok(())
    }

    async fn test_get_txs_from_faulty_node(
        nodes: &mut TestNodes,
        context: &mut IntegrationTestContext,
    ) -> Result<()> {
        let from = nodes.nodes[1].borrow_mut();
        from.kill();

        let err = get_cell_hashes(from.address).await.err();
        assert!(err.is_some());

        context.test_nodes.nodes[1].start();

        wait_until_nodes_start();

        context.count_test_run();

        Result::Ok(())
    }

    async fn send_cell_and_get_result(
        from: &TestNode,
        to: &TestNode,
        amount: u64,
        context: &mut IntegrationTestContext,
    ) -> Result<SendCellResult> {
        let cell = get_cell(amount, context, from).await?.unwrap();
        let cell_hash = cell.hash();
        let previous_output_len = cell.outputs().len();
        let previous_balance = get_outputs_capacity_of_owner(&cell, from);

        let spent_cell_hash = send_cell(from, to, cell, amount).await?;
        assert!(spent_cell_hash.is_some());

        // check that same tx was registered in all nodes
        let mut spent_cell: Option<Cell> = None;
        for node in &context.test_nodes.nodes {
            spent_cell = get_cell_from_hash(spent_cell_hash.unwrap().clone(), node.address).await?;
            assert!(spent_cell.is_some());
        }

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
        assert_eq!(
            spend_amount,
            spent_output.unwrap().capacity,
            "Invalid balance of the spent output"
        );

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

    struct SendCellResult {
        original_cell_balance: u64,
        original_cell_output_len: usize,
        original_cell_hash: CellHash,
        spent_cell: Cell,
    }

    fn run_nodes(nodes: &mut Vec<TestNode>) {
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

    fn wait_until_nodes_start() {
        sleep(Duration::from_secs(40));
    }
}

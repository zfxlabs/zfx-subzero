#[cfg(test)]
mod integration_test {
    use std::borrow::Borrow;
    use std::collections::HashMap;
    use std::hash::Hash;
    use std::net::SocketAddr;
    use std::thread::sleep;
    use std::time::Duration;

    use clap::{value_t, App, Arg};
    use ed25519_dalek::Keypair;
    use tracing::info;
    use tracing_subscriber;

    use crate::cell::transfer::TransferOperation;
    use crate::cell::types::CellHash;
    use crate::cell::Cell;

    use crate::protocol::Response;
    use crate::version;
    use crate::zfx_id::Id;
    use crate::Result;
    use crate::{client, sleet, Request};

    pub const KEYPAIR_NODE_0 : &str = "ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416";
    pub const KEYPAIR_NODE_1 : &str = "5a353c630d3faf8e2d333a0983c1c71d5e9b6aed8f4959578fbeb3d3f3172886393b576de0ac1fe86a4dd416cf032543ac1bd066eb82585f779f6ce21237c0cd";
    pub const KEYPAIR_NODE_2 : &str = "6f4b736b9a6894858a81696d9c96cbdacf3d49099d212213f5abce33da18716f067f8a2b9aeb602cd4163291ebbf39e0e024634f3be19bde4c490465d9095a6b";

    #[actix_rt::test]
    async fn test_generate_tx() -> Result<()> {
        let nodes = TestNodes::new();
        let node_0 = nodes.get_node(0).unwrap();
        let node_1 = nodes.get_node(1).unwrap();

        let cell_hash = get_cell_hashes(node_0.address).await?[2];
        let cell = get_cell_from_hash(cell_hash.clone(), node_0.address).await?;

        let spend_amount = 5;

        let transfer_op = TransferOperation::new(
            cell,
            node_0.public_key.clone(),
            node_1.public_key.clone(),
            spend_amount,
        );
        let transfer_tx = transfer_op.transfer(&node_0.keypair).unwrap();

        if let Some(Response::GenerateTxAck(ack)) = client::oneshot(
            node_0.address,
            Request::GenerateTx(sleet::GenerateTx { cell: transfer_tx }),
        )
        .await?
        {
            sleep(Duration::from_secs(2));
            let tx = get_cell_from_hash(ack.cell_hash.unwrap().clone(), node_0.address).await?;
            println!("value = {}", tx.outputs()[0].capacity);
            assert_eq!(spend_amount, tx.outputs()[0].capacity)
        } else {
            panic!("No acknowledgment received from sending the transaction");
        }

        Result::Ok(())
    }

    async fn get_cell_from_hash(cell_hash: CellHash, node_address: SocketAddr) -> Result<Cell> {
        if let Some(Response::CellAck(cell_ack)) = client::oneshot(
            node_address,
            Request::GetCell(sleet::GetCell { cell_hash: cell_hash.clone() }),
        )
        .await?
        {
            return Result::Ok(cell_ack.cell.expect("No transaction found for hash"));
        } else {
            panic!("Invalid response for request GetCell")
        }
    }

    async fn get_cell_hashes(node_address: SocketAddr) -> Result<Vec<CellHash>> {
        if let Some(Response::CellHashes(cell_hashes)) =
            client::oneshot(node_address, Request::GetCellHashes).await?
        {
            return Result::Ok(cell_hashes.ids);
        } else {
            panic!("Invalid response for request GetTransactions")
        }
    }

    pub struct TestNodes {
        nodes: Vec<TestNodeDetails>,
    }

    impl TestNodes {
        pub fn new() -> Self {
            let mut nodes = vec![];
            nodes.push(TestNodeDetails::new(0, KEYPAIR_NODE_0));
            nodes.push(TestNodeDetails::new(1, KEYPAIR_NODE_1));
            nodes.push(TestNodeDetails::new(2, KEYPAIR_NODE_2));

            TestNodes { nodes }
        }

        fn get_node(&self, id: usize) -> Option<&TestNodeDetails> {
            return self.nodes.get(id);
        }
    }

    pub struct TestNodeDetails {
        keypair: Keypair,
        public_key: [u8; 32],
        address: SocketAddr,
    }

    impl TestNodeDetails {
        pub fn new(id: u32, keypair: &str) -> Self {
            let (kp, pkh) = TestNodeDetails::create_keys_of_node(keypair);
            let mut address = String::from("127.0.0.1:123");
            address.push_str((id + 4).to_string().borrow());
            TestNodeDetails {
                keypair: kp,
                public_key: pkh,
                address: address.parse().expect("failed to construct address"),
            }
        }

        fn create_keys_of_node(keypair: &str) -> (Keypair, [u8; 32]) {
            let keypair_bytes = hex::decode(keypair).unwrap();
            let keypair = Keypair::from_bytes(&keypair_bytes).unwrap();
            let encoded = bincode::serialize(&keypair.public).unwrap();
            let pkh = blake3::hash(&encoded).as_bytes().clone();
            (keypair, pkh)
        }
    }
}

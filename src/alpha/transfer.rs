use super::Result;
use crate::cell::inputs::Inputs;
use crate::cell::outputs::{Output, Outputs};
use crate::cell::types::*;
use crate::cell::{Cell, CellType};

use crate::alpha::cell_operation::{consume_from_cell, ConsumeResult};
use ed25519_dalek::Keypair;

/// Empty transfer state - capacity transfers do not need to store extra state.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct TransferState;

/// A transfer output transfers tokens to the designated public key hash.
pub fn transfer_output(pkh: PublicKeyHash, capacity: Capacity) -> Result<Output> {
    let data = bincode::serialize(&TransferState {})?;
    Ok(Output { capacity, cell_type: CellType::Transfer, data, lock: pkh })
}

/// A transfer operation transfers capacity from an owner to another.
pub struct TransferOperation {
    /// The cell being spent in this transfer operation.
    cell: Cell,
    /// The recipient of the transferred capacity.
    recipient_address: PublicKeyHash,
    /// The recipient of the change capacity.
    change_address: PublicKeyHash,
    /// The amount of capacity to transfer.
    capacity: Capacity,
}

impl TransferOperation {
    pub fn new(
        cell: Cell,
        recipient_address: PublicKeyHash,
        change_address: PublicKeyHash,
        capacity: Capacity,
    ) -> Self {
        TransferOperation { cell, recipient_address, change_address, capacity }
    }

    /// Create a new set of transfer outputs from the supplied transfer operation.
    pub fn transfer(&self, keypair: &Keypair) -> Result<Cell> {
        let ConsumeResult { consumed, residue, inputs } =
            consume_from_cell(&self.cell, self.capacity, keypair)?;

        let main_output = transfer_output(self.recipient_address, consumed)?;
        let outputs = if residue > FEE && residue - FEE > 0 {
            vec![main_output, transfer_output(self.change_address, residue - FEE)?]
        } else {
            vec![main_output]
        };

        Ok(Cell::new(Inputs::new(inputs), Outputs::new(outputs)))
    }
}

#[cfg(test)]
mod test {
    use super::super::Error;
    use super::*;

    use crate::alpha::coinbase::CoinbaseOperation;

    use ed25519_dalek::Keypair;

    use std::convert::TryInto;

    #[actix_rt::test]
    async fn test_transfer_more_than_owner_output_has() {
        let (kp1, _kp2, pkh1, pkh2) = generate_keys();

        let coinbase_op = CoinbaseOperation::new(vec![(pkh2.clone(), 688), (pkh1.clone(), 120)]);
        let coinbase_tx = coinbase_op.try_into().unwrap();

        let transfer_op = TransferOperation::new(coinbase_tx, pkh2.clone(), pkh1.clone(), 133); // pkh1 does not have enough balance

        assert_eq!(transfer_op.transfer(&kp1), Err(Error::ExceedsAvailableFunds));
    }

    #[actix_rt::test]
    async fn test_transfer_with_total_less_than_fee() {
        let (kp1, _kp2, pkh1, pkh2) = generate_keys();

        let coinbase_op = CoinbaseOperation::new(vec![(pkh1.clone(), 1), (pkh1.clone(), 1)]);
        let coinbase_tx = coinbase_op.try_into().unwrap();

        let transfer_op = TransferOperation::new(coinbase_tx, pkh2.clone(), pkh1.clone(), 3);

        assert_eq!(transfer_op.transfer(&kp1), Err(Error::ExceedsAvailableFunds));
    }

    #[actix_rt::test]
    async fn test_transfer_with_three_outputs() {
        let (kp1, _kp2, pkh1, pkh2) = generate_keys();

        let coinbase_op = CoinbaseOperation::new(vec![
            (pkh1.clone(), 700),
            (pkh1.clone(), 1000),
            (pkh1.clone(), 300),
        ]);
        let coinbase_tx = coinbase_op.try_into().unwrap();

        let transfer_op = TransferOperation::new(coinbase_tx, pkh2.clone(), pkh1.clone(), 1800);
        let transfer_tx = transfer_op.transfer(&kp1).unwrap();

        assert_eq!(transfer_tx.inputs().len(), 3);
        assert_eq!(transfer_tx.outputs()[0].capacity, 200 - FEE); // total spent - fee
        assert_eq!(transfer_tx.outputs()[1].capacity, 1800); // remaining spendable amount
    }

    #[actix_rt::test]
    async fn test_transfer_zero_then_throw_error() {
        let (kp1, _kp2, pkh1, pkh2) = generate_keys();

        let coinbase_tx = generate_coinbase(&kp1, 1000);
        let transfer_op = TransferOperation::new(coinbase_tx, pkh2.clone(), pkh1.clone(), 0);
        assert_eq!(transfer_op.transfer(&kp1), Err(Error::ZeroTransfer));
    }

    #[actix_rt::test]
    async fn test_transfer_more_than_allowed_then_throw_error() {
        let (kp1, _kp2, pkh1, pkh2) = generate_keys();

        let coinbase_tx = generate_coinbase(&kp1, 1000);
        let transfer_op1 =
            TransferOperation::new(coinbase_tx.clone(), pkh2.clone(), pkh1.clone(), 1000);
        let transfer_op2 =
            TransferOperation::new(coinbase_tx.clone(), pkh2.clone(), pkh1.clone(), 1001 - FEE);
        assert_eq!(transfer_op1.transfer(&kp1), Err(Error::ExceedsAvailableFunds));
        // Should fail due to fee inclusion
        assert_eq!(transfer_op2.transfer(&kp1), Err(Error::ExceedsAvailableFunds));
    }

    #[actix_rt::test]
    async fn test_transfer_various_amounts() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        // Generate a coinbase transaction and spend it
        let coinbase_tx = generate_coinbase(&kp1, 1000);
        let transfer_op1 =
            TransferOperation::new(coinbase_tx, pkh2.clone(), pkh1.clone(), 1000 - FEE);

        let tx2 = transfer_op1.transfer(&kp1).unwrap();
        assert_eq!(tx2.inputs().len(), 1);
        assert_eq!(tx2.outputs().len(), 1);
        // The sum of the outputs should be 1000 - fee = 900
        assert_eq!(tx2.sum(), 1000 - FEE);

        // Spend the result of spending the coinbase. tx2 for pkh2 owner should have 900 spendable capacity
        let transfer_op2 = TransferOperation::new(tx2, pkh1.clone(), pkh2.clone(), 700);
        let tx3 = transfer_op2.transfer(&kp2).unwrap();
        assert_eq!(tx3.inputs().len(), 1);
        // The sum should take into account the change amount
        assert_eq!(tx3.sum(), 1000 - FEE * 2);

        // tx3 for pkh1 owner should have 700 - FEE spendable capacity
        let transfer_op3 = TransferOperation::new(tx3, pkh2.clone(), pkh1.clone(), 700 - FEE);
        let tx4 = transfer_op3.transfer(&kp1).unwrap();
        assert_eq!(tx4.sum(), 700 - FEE);
        assert_eq!(tx4.outputs().len(), 1);
    }

    fn generate_coinbase(keypair: &Keypair, amount: u64) -> Cell {
        let pkh = hash_public(keypair);
        let coinbase_op = CoinbaseOperation::new(vec![(pkh, amount)]);
        coinbase_op.try_into().unwrap()
    }

    fn hash_public(keypair: &Keypair) -> [u8; 32] {
        let enc = bincode::serialize(&keypair.public).unwrap();
        blake3::hash(&enc).as_bytes().clone()
    }

    fn generate_keys() -> (Keypair, Keypair, [u8; 32], [u8; 32]) {
        let kp1_hex = "ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416".to_owned();
        let kp2_hex = "5a353c630d3faf8e2d333a0983c1c71d5e9b6aed8f4959578fbeb3d3f3172886393b576de0ac1fe86a4dd416cf032543ac1bd066eb82585f779f6ce21237c0cd".to_owned();

        let kp1 = Keypair::from_bytes(&hex::decode(kp1_hex).unwrap()).unwrap();
        let kp2 = Keypair::from_bytes(&hex::decode(kp2_hex).unwrap()).unwrap();

        let pkh1 = hash_public(&kp1);
        let pkh2 = hash_public(&kp2);

        return (kp1, kp2, pkh1, pkh2);
    }
}

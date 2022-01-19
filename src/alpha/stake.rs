use crate::zfx_id::Id;

use crate::alpha::coinbase::CoinbaseState;
use crate::alpha::transfer::{self, TransferState};

use crate::cell::inputs::{Input, Inputs};
use crate::cell::outputs::{Output, Outputs};
use crate::cell::types::*;
use crate::cell::{Cell, CellType};

use super::{Error, Result};

use ed25519_dalek::Keypair;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct StakeState {
    pub node_id: Id,
}

/// A stake output locks tokens for a specific duration and can be used to stake on the network until
/// the time expires.
pub fn stake_output(node_id: Id, pkh: PublicKeyHash, capacity: Capacity) -> Result<Output> {
    let data = bincode::serialize(&StakeState { node_id })?;
    Ok(Output { capacity, cell_type: CellType::Stake, data, lock: pkh })
}

/// Checks that the output has the right form.
pub fn validate_output(output: Output) -> Result<()> {
    match output.cell_type {
        CellType::Coinbase => {
            let _: CoinbaseState = bincode::deserialize(&output.data)?;
            Ok(())
        }
        CellType::Transfer => {
            let _: TransferState = bincode::deserialize(&output.data)?;
            Ok(())
        }
        CellType::Stake => {
            let _: StakeState = bincode::deserialize(&output.data)?;
            Ok(())
        }
    }
}

pub struct StakeOperation {
    /// The cell being staked in this staking operation.
    cell: Cell,
    /// The node id of the validator (hash of the TLS certificate (trusted) / ip (untrusted)).
    node_id: Id,
    /// The address which receives the unstaked capacity.
    address: PublicKeyHash,
    /// The amount of capacity to stake.
    capacity: Capacity,
}

impl StakeOperation {
    pub fn new(cell: Cell, node_id: Id, address: PublicKeyHash, capacity: Capacity) -> Self {
        StakeOperation { cell, node_id, address, capacity }
    }

    pub fn stake(&self, keypair: &Keypair) -> Result<Cell> {
        let encoded_public = bincode::serialize(&keypair.public)?;
        let pkh = blake3::hash(&encoded_public).as_bytes().clone();

        self.validate_capacity(self.capacity.clone())?;

        let mut owned_outputs = vec![];
        for output in self.cell.outputs().iter() {
            // Validate the output to make sure it has the right form.
            let () = output.validate_capacity()?;
            let () = validate_output(output.clone())?;
            if output.lock == pkh.clone() {
                owned_outputs.push(output.clone());
            } else {
                continue;
            }
        }

        // Consume outputs and construct inputs - the remaining inputs should be reflected in the
        // change amount.
        let mut i = 0;
        let mut spending_capacity = self.capacity.clone();
        let mut change_capacity = 0;
        let mut consumed = 0;
        let mut inputs = vec![];
        if owned_outputs.len() > 0 {
            for output in owned_outputs.iter() {
                if consumed < self.capacity {
                    inputs.push(Input::new(keypair, self.cell.hash(), i)?);
                    if spending_capacity >= output.capacity {
                        spending_capacity -= output.capacity;
                        consumed += output.capacity;
                    } else {
                        consumed += spending_capacity;
                        change_capacity = output.capacity - spending_capacity;
                    }
                    i += 1;
                } else {
                    break;
                }
            }
        } else {
            return Err(Error::UnspendableCell);
        }

        // Create a change output.
        let main_output = stake_output(self.node_id.clone(), self.address.clone(), consumed)?;
        let outputs = if change_capacity > FEE && change_capacity - FEE > 0 {
            vec![
                main_output,
                transfer::transfer_output(self.address.clone(), change_capacity - FEE)?,
            ]
        } else {
            vec![main_output]
        };

        Ok(Cell::new(Inputs::new(inputs), Outputs::new(outputs)))
    }

    /// Checks that the capacity is > 0 and does not exceed the sum of the outputs.
    fn validate_capacity(&self, capacity: Capacity) -> Result<()> {
        let mut total = self.cell.sum();
        if capacity == 0 {
            return Err(Error::ZeroStake);
        }
        if capacity > total - FEE {
            return Err(Error::ExceedsAvailableFunds);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::alpha::coinbase::CoinbaseOperation;

    use crate::cell::Cell;

    use ed25519_dalek::Keypair;

    use std::convert::TryInto;

    #[actix_rt::test]
    async fn test_stake_more_than_allowed_then_throw_error() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        let c1 = generate_coinbase(&kp1, 1000);
        let stake_op1 = StakeOperation::new(c1.clone(), Id::generate(), pkh2, 1000);
        let stake_op2 = StakeOperation::new(c1, Id::generate(), pkh2, 1001 - FEE);
        assert_eq!(stake_op1.stake(&kp1), Err(Error::ExceedsAvailableFunds));
        assert_eq!(stake_op1.stake(&kp1), Err(Error::ExceedsAvailableFunds));
    }

    #[actix_rt::test]
    async fn test_stake() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        // Generate a coinbase transaction and stake it
        let c1 = generate_coinbase(&kp1, 1000);
        let stake_op1 = StakeOperation::new(c1.clone(), Id::generate(), pkh2, 900);
        let c2 = stake_op1.stake(&kp1).unwrap();

        assert_eq!(c2.inputs().len(), 1);
        assert_eq!(c2.outputs().len(), 1);
        // The sum of the outputs should be 900
        assert_eq!(c2.sum(), 1000 - FEE);

        // Stake half the amount in a coinbase tx
        let stake_op2 = StakeOperation::new(c1, Id::generate(), pkh1, 500);
        let c3 = stake_op2.stake(&kp1).unwrap();
        assert_eq!(c3.inputs().len(), 1);
        assert_eq!(c3.outputs().len(), 2);
        assert_eq!(c3.sum(), 1000 - FEE);
    }

    fn hash_public(keypair: &Keypair) -> [u8; 32] {
        let enc = bincode::serialize(&keypair.public).unwrap();
        blake3::hash(&enc).as_bytes().clone()
    }

    fn generate_coinbase(keypair: &Keypair, amount: u64) -> Cell {
        let pkh = hash_public(keypair);
        let coinbase_op = CoinbaseOperation::new(vec![(pkh, amount)]);
        coinbase_op.try_into().unwrap()
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

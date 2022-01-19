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
        self.validate_capacity(self.capacity.clone())?;

        // Consume outputs and construct inputs - the remaining inputs should be reflected in the
        // change amount.
        let mut i = 0;
        let mut spending_capacity = self.capacity.clone();
        let mut change_capacity = 0;
        let mut consumed = 0;
        let mut inputs = vec![];
        for output in self.cell.outputs().iter() {
            // Validate the output to make sure it has the right form.
            let () = output.validate_capacity()?;
            let () = validate_output(output.clone())?;
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

        // Create a change output.
        let main_output = stake_output(self.node_id.clone(), self.address.clone(), consumed)?;
        let outputs = if change_capacity > 0 {
            vec![main_output, transfer::transfer_output(self.address.clone(), change_capacity)?]
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

use super::cell_id::CellId;
use super::cell_type::CellType;
use super::types::{Capacity, CellHash, PublicKeyHash};
use super::{Error, Result};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Output {
    /// The capacity supplied by this cell output.
    pub capacity: Capacity,
    /// The type of data held within this output (generic).
    pub cell_type: CellType,
    /// The data held within this output (generic).
    pub data: Vec<u8>,
    /// The owner of the cell output (TODO: should be made generic).
    pub lock: PublicKeyHash,
}

impl Output {
    pub fn validate_capacity(&self) -> Result<()> {
        // TODO: Check that the cell capacity >= serialized(self)
        Ok(())
    }

    // /// Evaluates a cell output against the state. Note that when this is executed, the cell is
    // /// expected to have already undergone semantic verification.
    // pub fn evaluate<'a>(&'a self, state: &'a mut State, cell_hash: CellHash, i: u8) -> Result<()> {
    // 	match self.cell_type {
    // 	    CellType::Coinbase => {
    // 		// Coinbase output cells are only valid:
    // 		//   a) At genesis.
    // 		//   b) TODO: As rewards for validators whose end time is up.
    // 		if state.height == 0 {
    // 		    // Coinbase output cells create new spendable outputs.
    // 		    let cell_id = CellId::from_output(cell_hash.clone(), i, self.clone());
    // 		    state.cells.insert(cell_id, self.clone());
    // 		    // Coinbase output cells increase the total capacity of the network.
    // 		    state.total_capacity += self.capacity;
    // 		    Ok(())
    // 		} else {
    // 		    Err(Error::InvalidCoinbase)
    // 		}
    // 	    },
    // 	    CellType::Transfer => {
    // 		// Transfer output cells create new spendable outputs.
    // 		let cell_id = CellId::from_output(cell_hash.clone(), i, self.clone());
    // 		state.cells.insert(cell_id, self.clone());
    // 		// Transfer output cells increase the total capacity of the network.
    // 		state.total_capacity += self.capacity;
    // 		Ok(())
    // 	    },
    // 	    CellType::Stake => {
    // 		// FIXME: Stake output cells must be locked for the staking duration and produce
    // 		// coinbase transactions when the staking time is up as rewards.

    // 		// Stake output cells increase the staking capacity of the network.
    // 		state.total_staking_capacity += self;
    // 		Ok(())
    // 	    },
    // 	}
    // }
}

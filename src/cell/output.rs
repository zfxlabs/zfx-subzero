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

    // Applies a series of unspent output states to form a new output state.
    // pub fn apply<'a>(&'a self, outputs: Vec<Output>) -> Result<Output> {
    // 	match self.cell_type {
    // 	    CellType::Coinbase => {
    // 		// Coinbase output cells are only valid at genesis, produce the same cells and have
    // 		// no outputs.
    // 		if outputs.len() != 0 {
    // 		    return Err(Error::InvalidCoinbase);
    // 		}
    // 		if state.height == 0 {
    // 		    Ok(self.clone())
    // 		} else {
    // 		    Err(Error::InvalidCoinbase)
    // 		}
    // 	    },
    // 	    CellType::Transfer => {
    // 		// Transfer output cells produce the same cells and do not have specific state.
    // 		Ok(self.clone())
    // 	    },
    // 	    CellType::Stake => {
    // 		// FIXME: Stake cells create new state designating a `node_id` who is responsible for
    // 		// staking the capacity designated in the cell. Moreover the stake cell includes a
    // 		// staking start and end time.
    // 		Ok(self.clone())
    // 	    },
    // 	}
    // }

    // Verifies that the `data` in the current output is consistent with its consumed output cells.
    pub fn verify(&self, outputs: Vec<Output>) -> Result<()> {
        match self.cell_type {
            CellType::Coinbase => {
                // Coinbase operations do not consume other coinbase outputs.
                if outputs.len() != 0 {
                    return Err(Error::InvalidCoinbase);
                }
                // Coinbase operations are only valid at genesis.
                // if state.height != 0 {
                //     return Err(Error::InvalidCoinbase);
                // }
                Ok(())
            }
            // Besides checking the data field, transfer cell types are verified only as a valid cell.
            CellType::Transfer => Ok(()),
            CellType::Stake => {
                // Stake operations do not consume other stake outputs.
                if outputs.len() != 0 {
                    return Err(Error::InvalidStake);
                }
                // Stake operations must have a valid `node_id`.

                // Note: We leave these two to make it easier to test the network.
                // TODO: Stake operations are only valid after the start time.
                // TODO: Stake operations are only valid prior to the end time.
                Ok(())
            }
        }
    }
}

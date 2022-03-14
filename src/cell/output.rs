use crate::alpha::stake::StakeState;

use super::cell_type::CellType;
use super::types::{Capacity, PublicKeyHash};
use super::{Error, Result};

use crate::colored::Colorize;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
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

impl std::fmt::Debug for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.cell_type {
            CellType::Coinbase => {
                let lock = hex::encode(self.lock);
                write!(f, "coinbase (⚴ {}) = {}", lock, self.capacity)
            }
            CellType::Transfer => {
                let lock = hex::encode(self.lock);
                write!(f, "transfer (⚴ {}) = {}", lock, self.capacity)
            }
            CellType::Stake => {
                let state: StakeState = bincode::deserialize(&self.data).unwrap();
                let lock = hex::encode(self.lock);
                write!(f, "stake {} (⚴ {}) = {}", state.node_id, lock, self.capacity)
            }
        }
    }
}

impl std::fmt::Display for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.cell_type {
            CellType::Coinbase => {
                let capacity = format!("{}", self.capacity).magenta();
                write!(f, "{} = {}", "coinbase".cyan(), capacity)
            }
            CellType::Transfer => {
                let capacity = format!("{}", self.capacity).magenta();
                write!(f, "{} = {}", "transfer".cyan(), capacity)
            }
            CellType::Stake => {
                let state: StakeState = bincode::deserialize(&self.data).unwrap();
                let capacity = format!("{}", self.capacity).magenta();
                let node_id = format!("{}", state.node_id).yellow();
                write!(f, "{} {} = {}", "stake".cyan(), node_id, capacity)
            }
        }
    }
}

impl Output {
    pub fn validate_capacity(&self) -> Result<()> {
        // TODO: Check that the cell capacity >= serialized(self)
        Ok(())
    }

    // Applies a series of unspent output states to form a new output state.
    pub fn apply<'a>(&'a self, outputs: Vec<Output>) -> Result<Output> {
        match self.cell_type {
            CellType::Coinbase => {
                // Coinbase output cells are primitives which cannot be applied to other cells `data`.
                if outputs.len() != 0 {
                    return Err(Error::InvalidCoinbase);
                }
                Ok(self.clone())
            }
            CellType::Transfer => {
                // Transfer output cells are primitives which cannot be applied to other cells `data`.
                Ok(self.clone())
            }
            CellType::Stake => {
                // Stake output cells are primitives which cannot be applied to other cells `data`.
                if outputs.len() != 0 {
                    return Err(Error::InvalidStake);
                }
                Ok(self.clone())
            }
        }
    }

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
                // Note: We leave these temporarily to make it easier to test the network.
                // TODO: Stake operations must have a valid `node_id`.
                // TODO: Stake operations are only valid after the start time.
                // TODO: Stake operations are only valid prior to the end time.
                Ok(())
            }
        }
    }
}

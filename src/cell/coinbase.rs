use super::cell::Cell;
use super::cell_type::CellType;
use super::inputs::{Input, Inputs};
use super::outputs::{Output, Outputs};
use super::types::*;
use super::{Error, Result};

use std::convert::TryInto;

/// Empty coinbase state - coinbases do not need to store extra state.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct CoinbaseState;

pub fn coinbase_output(recipient_address: PublicKeyHash, capacity: Capacity) -> Result<Output> {
    let data = bincode::serialize(&CoinbaseState {})?;
    Ok(Output { capacity, cell_type: CellType::Coinbase, data, lock: recipient_address })
}

pub struct CoinbaseOperation {
    recipients: Vec<(PublicKeyHash, Capacity)>,
}

impl CoinbaseOperation {
    pub fn new(recipients: Vec<(PublicKeyHash, Capacity)>) -> Self {
        CoinbaseOperation { recipients }
    }
}

impl TryInto<Cell> for CoinbaseOperation {
    type Error = Error;

    fn try_into(self) -> Result<Cell> {
        let mut outputs = vec![];
        for (pkh, capacity) in self.recipients.iter().cloned() {
            outputs.push(coinbase_output(pkh, capacity)?);
        }
        Ok(Cell::new(Inputs::new(vec![]), Outputs::new(outputs)))
    }
}

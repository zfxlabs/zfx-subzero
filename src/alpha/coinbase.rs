use super::{Error, Result};
use crate::cell::inputs::Inputs;
use crate::cell::outputs::{Output, Outputs};
use crate::cell::types::*;
use crate::cell::{Cell, CellType};

use std::convert::TryInto;

/// Empty coinbase state - coinbases do not need to store extra state.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct CoinbaseState;

/// Create a new [Output] of type [CellType::Coinbase] by setting
/// the indicated capacity to the account.
/// The output will have `data` property as a serialized [coinbase state][CoinbaseState].
///
/// ## Parameters
/// * `recipient_address` - public key hash of account which will get the tokens
/// * `capacity`- starting balance for the account
pub fn coinbase_output(recipient_address: PublicKeyHash, capacity: Capacity) -> Result<Output> {
    let data = bincode::serialize(&CoinbaseState {})?;
    Ok(Output { capacity, cell_type: CellType::Coinbase, data, lock: recipient_address })
}

/// Creates a coinbase from a list of balances for each account.
pub struct CoinbaseOperation {
    /// A list of balances per account to create a coinbase [Cell]
    recipients: Vec<(PublicKeyHash, Capacity)>,
}

impl CoinbaseOperation {
    /// Create a coinbase operation from a list of balances for each account's public keys.
    ///
    /// The method [try_into][TryInto::try_into] should be called to complete the construction of
    /// [Cell] with [coinbase][CellType::Coinbase] [outputs][Output].
    ///
    /// ## Parameters
    /// * `recipients` - a list of accounts and their balances
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

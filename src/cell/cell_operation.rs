use crate::alpha::coinbase::CoinbaseState;
use crate::alpha::stake::StakeState;
use crate::alpha::transfer::TransferState;
use crate::alpha::{Error, Result};
use crate::cell::inputs::Input;
use crate::cell::outputs::Output;
use crate::cell::types::{Capacity, FEE};
use crate::cell::{Cell, CellType};
use ed25519_dalek::Keypair;

/// A response from [consume_from_cell]
pub struct ConsumeResult {
    /// Consumed amount from [Cell]
    pub consumed: Capacity,
    /// Remaining balance in [Cell]
    pub residue: Capacity,
    /// New [inputs][Input] composed from consumed [outputs][Output] of [Cell]
    pub inputs: Vec<Input>,
}

/// Take `amount` from the `cell` [outputs][Output], belonging to the owner with `owner_key`.
/// For each owner's output, it will:
/// 1. reduce it's remaining amount until the required amount is fully consumed.
/// 2. create a new [Input], for each [Output] affected by 1st operation above.
///    Each input will contain the owner's key, a hash of the original cell
///    and index of the output in the outputs list of the cell.
///
/// Returns a structure with a list of new [inputs][Input], consumed and
/// remaining balances of the outputs.
///
/// Throws the following errors:
/// * [Error::UnspendableCell] - if no outputs found in the cell for the owner
/// * [Error::ZeroTransfer] - if attempting to spend 0 amount
/// * [Error::ExceedsAvailableFunds] - if outputs of the owner has not enough balance + [FEE]
/// comparing to the requested `amount` to consume.
///
/// ## Properties
/// * `cell` - a cell to consume the `amount` from.
/// * `amount` - `amount` to take out from the `cell`.
/// * `owner_key` - owner's keypair is used to identify [outputs][Output]
/// where `amount` can be taken from.
pub fn consume_from_cell(
    cell: &Cell,
    amount: Capacity,
    owner_key: &Keypair,
) -> Result<ConsumeResult> {
    let encoded_public = bincode::serialize(&owner_key.public)?;
    let pkh = blake3::hash(&encoded_public).as_bytes().clone();

    let mut owned_outputs = vec![];
    let mut output_indices = vec![];
    let mut output_index: u8 = 0;
    for output in cell.outputs().iter() {
        // Validate the output to make sure it has the right form.
        let () = output.validate_capacity()?;
        let () = validate_output(output.clone())?;
        if output.lock == pkh.clone() {
            owned_outputs.push(output.clone());
            output_indices.push(output_index);
        }
        output_index += 1;
    }

    validate_capacity(&owned_outputs, amount, FEE)?;

    // Consume outputs and construct inputs - the remaining inputs should be reflected in the
    // change amount.
    let mut i = 0;
    let mut spending_capacity = amount;
    let mut residue = 0;
    let mut consumed = 0;
    let mut inputs = vec![];
    if owned_outputs.len() > 0 {
        for output in owned_outputs.iter() {
            if consumed < amount {
                inputs.push(Input::new(owner_key, cell.hash(), output_indices[i])?);
                if spending_capacity >= output.capacity {
                    spending_capacity -= output.capacity;
                    consumed += output.capacity;
                } else {
                    consumed += spending_capacity;
                    residue = output.capacity - spending_capacity;
                }
                i += 1;
            } else {
                break;
            }
        }
    } else {
        return Err(Error::UnspendableCell);
    }

    Ok(ConsumeResult { consumed, residue, inputs })
}

/// Checks that the output has the right form.
fn validate_output(output: Output) -> Result<()> {
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

/// Checks that the capacity is > 0 and does not exceed the sum of the outputs.
fn validate_capacity(outputs: &Vec<Output>, capacity: Capacity, fee: u64) -> Result<()> {
    let total: u64 = outputs.iter().map(|o| o.capacity).sum();
    if capacity == 0 {
        return Err(Error::ZeroTransfer);
    }
    if total < fee || capacity > total - fee {
        return Err(Error::ExceedsAvailableFunds);
    }
    Ok(())
}

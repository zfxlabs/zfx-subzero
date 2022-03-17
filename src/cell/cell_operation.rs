use crate::alpha::coinbase::CoinbaseState;
use crate::alpha::stake::StakeState;
use crate::alpha::transfer::TransferState;
use crate::alpha::{Error, Result};
use crate::cell::inputs::Input;
use crate::cell::outputs::Output;
use crate::cell::types::{Capacity, FEE};
use crate::cell::{Cell, CellType};
use ed25519_dalek::Keypair;

pub struct ConsumeResult {
    pub consumed: Capacity,
    pub residue: Capacity,
    pub inputs: Vec<Input>,
}

pub fn consume_from_cell(
    cell: &Cell,
    amount: Capacity,
    owner_key: &Keypair,
) -> Result<ConsumeResult> {
    let encoded_public = bincode::serialize(&owner_key.public)?;
    let pkh = blake3::hash(&encoded_public).as_bytes().clone();

    let mut owned_outputs = vec![];
    for output in cell.outputs().iter() {
        // Validate the output to make sure it has the right form.
        let () = output.validate_capacity()?;
        let () = validate_output(output.clone())?;
        if output.lock == pkh.clone() {
            owned_outputs.push(output.clone());
        } else {
            continue;
        }
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
                inputs.push(Input::new(owner_key, cell.hash(), i)?);
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

/// Checks that the capacity is > 0 and does not exceed the sum of the outputs.
pub fn validate_capacity(outputs: &Vec<Output>, capacity: Capacity, fee: u64) -> Result<()> {
    let total: u64 = outputs.iter().map(|o| o.capacity).sum();
    if capacity == 0 {
        return Err(Error::ZeroTransfer);
    }
    if total < fee || capacity > total - fee {
        return Err(Error::ExceedsAvailableFunds);
    }
    Ok(())
}

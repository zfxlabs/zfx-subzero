use super::inputs::Inputs;
use super::outputs::{Output, Outputs};
use super::types::*;

/// Cell is an extension to the UTXO model used by [sleet][crate::sleet] and [hail][crate::hail] components
/// when they interact with transactions by wrapping it inside [transactions](crate::sleet::tx::Tx).
///
/// The main information withing a transaction is stored in its 2 properties:
/// [inputs][crate::cell::input::Input] and [outputs][crate::cell::output::Output].
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Cell {
    inputs: Inputs,
    outputs: Outputs,
}

impl std::fmt::Display for Cell {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "inputs: {}\noutputs: {}\n", self.inputs, self.outputs)
    }
}

impl Cell {
    /// Create new instance from `inputs` and `outputs`.
    pub fn new(inputs: Inputs, outputs: Outputs) -> Self {
        Cell { inputs, outputs }
    }

    /// Return sorted cloned `inputs`.
    pub fn inputs(&self) -> Inputs {
        self.inputs.clone()
    }

    /// Return sorted cloned `outputs`.
    pub fn outputs(&self) -> Outputs {
        self.outputs.clone()
    }

    /// Return all outputs of the `owner` in the cell. The function checks [Output::lock] property
    /// whether it equals to `owner`.
    ///
    /// ## Parameters
    /// * `owner` - owner (account's public key) to retrieve the outputs
    pub fn outputs_of_owner(&self, owner: &PublicKeyHash) -> Vec<&Output> {
        self.outputs
            .iter()
            .filter_map(|o| if o.lock == *owner { Some(o) } else { None })
            .collect::<Vec<&Output>>()
    }

    /// Serialize the cell and return its hash as a byte array.
    pub fn hash(&self) -> CellHash {
        let encoded = bincode::serialize(self).unwrap();
        blake3::hash(&encoded).as_bytes().clone()
    }

    /// Sums the output capacities.
    pub fn sum(&self) -> Capacity {
        self.outputs().sum()
    }

    // pub fn semantic_verify(&self, cells: &HashMap<CellIds, Cell>) -> Result<()> {
    // 	let cell_ids = CellIds::from_inputs(&self.inputs);
    // 	Ok(())
    // }
}

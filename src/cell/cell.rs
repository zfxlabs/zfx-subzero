use super::cell_id::CellId;
use super::inputs::Inputs;
use super::outputs::{Output, Outputs};
use super::types::*;

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
    pub fn new(inputs: Inputs, outputs: Outputs) -> Self {
        Cell { inputs, outputs }
    }

    pub fn inputs(&self) -> Inputs {
        self.inputs.clone()
    }

    pub fn outputs(&self) -> Outputs {
        self.outputs.clone()
    }

    pub fn outputs_of_owner(&self, owner: &PublicKeyHash) -> Vec<&Output> {
        self.outputs
            .iter()
            .filter_map(|o| if o.lock == *owner { Some(o) } else { None })
            .collect::<Vec<&Output>>()
    }

    pub fn hash(&self) -> CellHash {
        let encoded = bincode::serialize(self).unwrap();
        blake3::hash(&encoded).as_bytes().clone()
    }

    pub fn id(&self) -> CellId {
        CellId::new(self.hash())
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

use super::inputs::{Input, Inputs};
use super::outputs::{Output, Outputs};
use super::types::*;

use std::collections::HashMap;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Cell {
    inputs: Inputs<Input>,
    outputs: Outputs<Output>,
}

impl Cell {
    pub fn new(inputs: Inputs<Input>, outputs: Outputs<Output>) -> Self {
        Cell { inputs, outputs }
    }

    pub fn inputs(&self) -> Inputs<Input> {
        self.inputs.clone()
    }

    pub fn outputs(&self) -> Outputs<Output> {
        self.outputs.clone()
    }

    pub fn hash(&self) -> CellHash {
        let encoded = bincode::serialize(self).unwrap();
        blake3::hash(&encoded).as_bytes().clone()
    }

    /// Sums the output capacities.
    pub fn sum(&self) -> Capacity {
        let mut total = 0;
        for output in self.outputs().iter() {
            total += output.capacity;
        }
        total
    }

    // pub fn semantic_verify(&self, cells: &HashMap<CellIds, Cell>) -> Result<()> {
    // 	let cell_ids = CellIds::from_inputs(&self.inputs);
    // 	Ok(())
    // }
}

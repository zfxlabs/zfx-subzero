use crate::zfx_id::Id;

use super::block::Block;
use super::stake::StakeState;
use super::{Error, Result};

use crate::cell::outputs::{Output, Outputs};
use crate::cell::types::{Capacity, PublicKeyHash};
use crate::cell::{Cell, CellId, CellIds, CellType};

use crate::colored::Colorize;
use crate::graph::dependency_graph::DependencyGraph;

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct State {
    /// The current block height.
    pub height: u64,
    /// The total spending capacity of the network.
    pub total_spending_capacity: Capacity,
    /// The total capacity currently staked in the network.
    pub total_staking_capacity: Capacity,
    /// The current validator set.
    pub validators: Vec<(Id, Capacity)>,
    /// A mapping of a cell ids (inputs) to unspent cell outputs.
    pub live_cells: HashMap<CellIds, Cell>,
}

impl State {
    pub fn new() -> Self {
        State {
            height: 0,
            total_spending_capacity: 0,
            total_staking_capacity: 0,
            validators: vec![],
            live_cells: HashMap::default(),
        }
    }

    pub fn apply(&self, block: Block) -> Result<State> {
        let mut state = self.clone();

        // Build a dependency graph from the blocks cells.
        let mut dg = DependencyGraph::new();
        for cell in block.cells.iter() {
            dg.insert(cell.clone())?;
        }
        let ordered_cells = dg.topological_cells(block.cells.clone())?;

        // Try to apply the cells by order of dependence.
        for cell in ordered_cells.iter() {
            // Pull all the live cell outputs which are inputs to the cell.
            let input_cell_ids = CellIds::from_inputs(cell.inputs())?;
            let mut consumed_cell_ids = CellIds::empty();
            let mut consumed_cell_outputs = vec![];
            let mut consumed_capacity = 0u64;
            let mut intersecting_cell_ids = CellIds::empty();
            for (live_cell_ids, live_cell) in state.live_cells.iter() {
                // println!("live_cell_ids = {:?}", live_cell_ids.clone());
                if input_cell_ids.intersects_with(live_cell_ids) {
                    // Fetch the intersecting cell ids.
                    let intersection = input_cell_ids.intersect(&live_cell_ids);
                    // println!("intersection = {:?}", intersection.clone());
                    // Fetch the outputs corresponding to the intersection.
                    let live_cell_outputs = live_cell.outputs();
                    for i in 0..live_cell_outputs.len() {
                        let cell_id = CellId::from_output(
                            live_cell.hash(),
                            i as u8,
                            live_cell_outputs[i].clone(),
                        )?;
                        if intersection.contains(&cell_id) {
                            consumed_cell_ids.insert(cell_id.clone());
                            consumed_cell_outputs.push(live_cell_outputs[i].clone());
                            consumed_capacity += live_cell_outputs[i].capacity;
                        }
                    }
                }
            }
            if consumed_cell_ids.clone() != input_cell_ids.clone() {
                // println!("consumed {:?}", consumed_cell_ids.clone());
                // println!("inputs {:?}", input_cell_ids.clone());
                return Err(Error::UndefinedCellIds);
            }

            // Verify that the cell outputs transition correctly according to their constraints.
            let mut verified_outputs = vec![];
            for output in cell.outputs().iter() {
                // Fetch consumed outputs of the same type as this output.
                let mut arguments = vec![];
                for consumed_output in consumed_cell_outputs.iter() {
                    if output.cell_type == consumed_output.cell_type {
                        arguments.push(consumed_output.clone());
                    }
                    let verified_output = output.verify(arguments.clone())?;
                    verified_outputs.push(verified_output);
                }
            }

            // Remove consumed output cells from the live cell map.
            state.remove_intersection(consumed_cell_ids);

            // Apply the primitive cell types which change the `alpha` state.
            let mut coinbase_capacity = 0u64;
            let mut produced_staking_capacity = 0u64;
            let mut produced_capacity = 0u64;
            let cell_outputs = cell.outputs();
            for i in 0..cell_outputs.len() {
                let cell_output = cell_outputs[i].clone();
                // If the cell output is a coinbase at genesis then add the produced capacity.
                if cell_output.cell_type == CellType::Coinbase {
                    if state.height == 0 {
                        // The coinbase generates capacity without consuming it.
                        coinbase_capacity += cell_output.capacity;
                    } else {
                        return Err(Error::InvalidCoinbase);
                    }
                } else if cell_output.cell_type == CellType::Stake {
                    // If the cell output is a `Stake` cell then add the validator to the list of
                    // validators.
                    let stake_state: StakeState = bincode::deserialize(&cell_output.data)?;
                    state.validators.push((stake_state.node_id, cell_output.capacity));
                    produced_staking_capacity += cell_output.capacity;
                } else {
                    // Otherwise treat it normally.
                    produced_capacity += cell_output.capacity;
                }
            }

            // Add newly produced output cells to live cell map.
            let produced_cell_ids = CellIds::from_outputs(cell.hash(), cell.outputs())?;
            // println!("inserting {:?}", produced_cell_ids);
            if let Some(_) = state.live_cells.insert(produced_cell_ids, cell.clone()) {
                return Err(Error::ExistingCellIds);
            }

            // Subtract the consumed capacity and add the produced capacity.
            if consumed_capacity >= produced_capacity + produced_staking_capacity
                && consumed_capacity > 0
                && coinbase_capacity == 0
            {
                // println!("consumed capacity = {:?}", consumed_capacity);
                // println!("total_spending_capacity = {:?}", state.total_spending_capacity);
                // println!("produced_capaciy = {:?}", produced_capacity);
                // println!("produced_staking_capacity = {:?}", produced_staking_capacity);
                state.total_spending_capacity -= consumed_capacity;
                state.total_spending_capacity += produced_capacity;
                state.total_staking_capacity += produced_staking_capacity;
            } else if state.height == 0
                && coinbase_capacity > 0
                && produced_capacity == 0
                && produced_staking_capacity == 0
            {
                // println!("coinbase capacity = {:?}", coinbase_capacity);
                state.total_spending_capacity += coinbase_capacity;
            } else {
                return Err(Error::ExceedsCapacity);
            }
        }
        Ok(state)
    }

    fn remove_intersection(&self, cell_ids: CellIds) -> Result<HashMap<CellIds, Cell>> {
        let mut live_cells = HashMap::default();
        for (live_cell_ids, live_cell) in self.live_cells.iter() {
            if cell_ids.intersects_with(live_cell_ids) {
                let intersection = cell_ids.intersect(&live_cell_ids);
                let new_cell_ids = live_cell_ids.left_difference(&intersection);
                if let Some(_) = live_cells.insert(new_cell_ids.clone(), live_cell.clone()) {
                    return Err(Error::ExistingCellIds);
                }
            } else {
                if let Some(_) = live_cells.insert(live_cell_ids.clone(), live_cell.clone()) {
                    return Err(Error::ExistingCellIds);
                }
            }
        }
        Ok(live_cells)
    }

    pub fn format(&self) -> String {
        let total_spending_capacity = format!("Σ = {:?}", self.total_spending_capacity).cyan();
        let mut s: String = format!("{}\n", total_spending_capacity);
        for (id, w) in self.validators.clone() {
            let id_s = format!("{:?}", id).yellow();
            let w_s = format!("{:?}", w).magenta();
            s = format!("{} ν = {} {} | {} {}\n", s, "⦑".cyan(), id_s, w_s, "⦒".cyan());
        }
        s
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::alpha::block;
    use crate::alpha::coinbase::CoinbaseOperation;
    use crate::alpha::initial_staker::InitialStaker;
    use crate::alpha::transfer::TransferOperation;

    use crate::zfx_id::Id;

    use std::convert::TryInto;
    use std::net::SocketAddr;

    use ed25519_dalek::Keypair;

    #[actix_rt::test]
    async fn test_apply_genesis() {
        let state = State::new();
        let block = block::build_genesis().unwrap();
        let produced_state = state.apply(block).unwrap();
        assert_eq!(produced_state.total_spending_capacity, 3000);
        assert_eq!(produced_state.total_staking_capacity, 3000);
    }

    fn initial_stakers() -> Vec<InitialStaker> {
        vec![
	    InitialStaker::from_hex(
		"ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416".to_owned(),
		Id::from_ip(&"127.0.0.1:1234".parse().unwrap()),
		2000, // 2000 allocated
		1000, // half of it staked so that we can transfer funds later
	    ).unwrap(),
	    InitialStaker::from_hex(
		"5a353c630d3faf8e2d333a0983c1c71d5e9b6aed8f4959578fbeb3d3f3172886393b576de0ac1fe86a4dd416cf032543ac1bd066eb82585f779f6ce21237c0cd".to_owned(),
		Id::from_ip(&"127.0.0.1:1235".parse().unwrap()),
		2000,
		1000,
	    ).unwrap(),
	    InitialStaker::from_hex(
		"6f4b736b9a6894858a81696d9c96cbdacf3d49099d212213f5abce33da18716f067f8a2b9aeb602cd4163291ebbf39e0e024634f3be19bde4c490465d9095a6b".to_owned(),
		Id::from_ip(&"127.0.0.1:1236".parse().unwrap()),
		2000,
		1000,
	    ).unwrap(),
	]
    }
}

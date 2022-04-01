use super::{Error, Result};

use crate::cell::types::CellHash;
use crate::cell::{Cell, CellId, CellIds};

use crate::sleet::conflict_set::ConflictSet;
use crate::sleet::BETA2;

use std::collections::{hash_map::Entry, HashMap, HashSet};

pub struct ConflictGraph {
    vertices: HashMap<CellId, VertexData>,
    cells: HashMap<CellHash, Cell>,
    cs: HashMap<CellHash, ConflictSet<CellHash>>,
}

struct VertexData {
    // parent_cell: CellHash,
    spenders: HashSet<CellHash>,
    accepted: bool,
}

impl ConflictGraph {
    pub fn new(genesis: CellIds) -> Self {
        let mut vertices = HashMap::new();
        for g in genesis.iter() {
            vertices.insert(g.clone(), VertexData { spenders: HashSet::new(), accepted: true });
        }
        ConflictGraph { vertices, cells: HashMap::new(), cs: HashMap::new() }
    }

    pub fn insert_cell(&mut self, cell: Cell) -> Result<()> {
        let cell_hash = cell.hash();
        match self.cells.insert(cell_hash, cell.clone()) {
            None => (),
            Some(_cell) => return Err(Error::DuplicateCell),
        }

        let produced_cell_ids = CellIds::from_outputs(cell_hash, cell.outputs())?;
        for cell_id in produced_cell_ids.iter() {
            self.vertices
                .insert(cell_id.clone(), VertexData { spenders: HashSet::new(), accepted: false });
        }

        let mut conflicts = HashSet::new();
        let consumed_cell_ids = CellIds::from_inputs(cell.inputs())?;
        for cell_id in consumed_cell_ids.iter() {
            match self.vertices.entry(cell_id.clone()) {
                Entry::Occupied(mut o) => {
                    let data = o.get_mut();
                    conflicts.extend(data.spenders.iter().cloned());
                    data.spenders.insert(cell_hash);
                }
                Entry::Vacant(_) => return Err(Error::UndefinedCell),
            }
        }

        let mut pref = None;
        let mut last = None;
        let mut cnt = 0u8;
        let mut own_cset = ConflictSet::new(cell_hash);
        for conflict_hash in conflicts.iter() {
            let set = self.cs.get_mut(conflict_hash).unwrap();
            if pref.is_none() {
                pref = Some(set.pref.clone());
                last = Some(set.last.clone());
                cnt = set.cnt;
            }

            set.conflicts.insert(cell_hash);
            own_cset.conflicts.insert(*conflict_hash);
        }
        if conflicts.len() > 0 {
            own_cset.pref = pref.unwrap();
            // FIXME: Not sure here.
            own_cset.last = last.unwrap();
            own_cset.cnt = cnt;
        }
        self.cs.insert(cell_hash, own_cset);

        Ok(())
    }

    pub fn accept_cell(&mut self, cell: Cell) -> Result<Vec<CellHash>> {
        let cell_hash = cell.hash();
        let mut conflicting_hashes = HashSet::new();
        match self.conflicting_cells(&cell_hash).cloned() {
            Some(conflict_set) => {
                let conflicts = conflict_set.conflicts.clone();
                let mut conflicts2 = HashSet::new();
                let consumed_cell_ids = CellIds::from_inputs(cell.inputs())?;
                for cell_id in consumed_cell_ids.iter() {
                    conflicts2.extend(self.vertices.get(cell_id).unwrap().spenders.iter());
                    self.vertices.remove(cell_id);
                }
                assert_eq!(conflicts, conflicts2);
                for conflict_hash in conflicts.iter() {
                    if cell_hash.eq(conflict_hash) {
                        continue;
                    }
                    self.cells.remove(conflict_hash);
                    self.cs.remove(conflict_hash);
                    let _ = conflicting_hashes.insert(conflict_hash.clone());
                }

                let mut new_cset = ConflictSet::new(cell_hash);
                // Retain the old confidence value for the new (singleton) conflict set
                new_cset.cnt = conflict_set.cnt;
                self.cs.insert(cell_hash, new_cset);

                Ok(conflicting_hashes.iter().map(|h| *h).collect())
            }
            // If the transaction has no conflict set then it is invalid.
            None => Err(Error::UndefinedCell),
        }
    }

    /// Remove a cell from the conflict graph
    pub fn remove_cell(&mut self, cell: Cell) -> Result<()> {
        // TODO
        Ok(())
    }

    pub fn conflicting_cells(&self, cell_hash: &CellHash) -> Option<&ConflictSet<CellHash>> {
        self.cs.get(cell_hash)
    }

    pub fn is_singleton(&self, cell_hash: &CellHash) -> Result<bool> {
        match self.conflicting_cells(cell_hash) {
            Some(conflict_set) => Ok(conflict_set.is_singleton()),
            None => Err(Error::UndefinedCellHash(cell_hash.clone())),
        }
    }

    pub fn get_preferred(&self, cell_hash: &CellHash) -> Result<CellHash> {
        match self.conflicting_cells(cell_hash) {
            Some(conflict_set) => Ok(conflict_set.pref),
            None => Err(Error::UndefinedCellHash(cell_hash.clone())),
        }
    }

    pub fn is_preferred(&self, cell_hash: &CellHash) -> Result<bool> {
        match self.conflicting_cells(cell_hash) {
            Some(conflict_set) => Ok(conflict_set.is_preferred(cell_hash.clone())),
            None => Err(Error::UndefinedCellHash(cell_hash.clone())),
        }
    }

    pub fn get_confidence(&self, cell_hash: &CellHash) -> Result<u8> {
        match self.conflicting_cells(cell_hash) {
            Some(conflict_set) => Ok(conflict_set.cnt),
            None => Err(Error::UndefinedCellHash(cell_hash.clone())),
        }
    }

    pub fn update_conflict_set(&mut self, cell_hash: &CellHash, d1: u8, d2: u8) -> Result<()> {
        if self.cs.len() > 0 {
            match self.cs.get_mut(cell_hash) {
                Some(cs) => {
                    if d1 > d2 {
                        cs.pref = cell_hash.clone();
                    }
                    if !cell_hash.eq(&cs.last) {
                        cs.last = cell_hash.clone();
                    } else {
                        if cs.cnt < BETA2 {
                            cs.cnt += 1;
                        }
                    }
                    Ok(())
                }
                None => Err(Error::UndefinedCellHash(cell_hash.clone())),
            }
        } else {
            Err(Error::EmptyConflictGraph)
        }
    }

    /// Reset the confidence counter for a cell
    pub fn reset_count(&mut self, cell_hash: &CellHash) -> Result<()> {
        if self.cs.len() > 0 {
            match self.cs.get_mut(cell_hash) {
                Some(cs) => {
                    cs.cnt = 0;
                    Ok(())
                }
                None => Err(Error::UndefinedCellHash(cell_hash.clone())),
            }
        } else {
            Err(Error::EmptyConflictGraph)
        }
    }

    /// Returns the number of cells in the conflict graph
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.cs.len()
    }
}

#[cfg(test)]
mod test {
    use super::ConflictGraph;

    use crate::alpha::coinbase::CoinbaseOperation;
    use crate::alpha::transfer;

    use crate::cell::inputs::{Input, Inputs};
    use crate::cell::outputs::Outputs;
    use crate::cell::types::{Capacity, CellHash};
    use crate::cell::{Cell, CellIds};

    use std::collections::HashSet;
    use std::convert::TryInto;

    use ed25519_dalek::Keypair;
    use rand::{thread_rng, Rng};

    #[actix_rt::test]
    async fn test_conflict_graph_with_many_cells() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        // Some root unspent outputs for `genesis`. We assume this input refers to a cell with funds
        // but for the purposes of the conflict graph it doesn't matter.
        let genesis_op = CoinbaseOperation::new(vec![
            (pkh1.clone(), 1000),
            (pkh2.clone(), 1000),
            (pkh2.clone(), 500),
        ]);
        let genesis_tx: Cell = genesis_op.try_into().unwrap();

        let mut dh: ConflictGraph = ConflictGraph::new(
            CellIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs()).unwrap().clone(),
        );

        let inputs = vec![
            Input::new(&kp1, genesis_tx.hash(), 0).unwrap(),
            Input::new(&kp2, genesis_tx.hash(), 1).unwrap(),
            Input::new(&kp2, genesis_tx.hash(), 2).unwrap(),
        ];
        let mut origin_txs = vec![];
        let mut original_spent_amounts = vec![];

        // spend a cell with each input once to have non-conflicting cells
        for i in 0..inputs.len() {
            let amount = (10 + i) as Capacity;
            // A transaction that spends `genesis` and produces a new output for `pkh2`.
            let tx = Cell::new(
                Inputs::new(vec![inputs[i].clone()]),
                Outputs::new(vec![transfer::transfer_output(pkh2.clone(), amount).unwrap()]),
            );
            dh.insert_cell(tx.clone()).unwrap();
            let tx_hash = tx.hash();
            let c = dh.conflicting_cells(&tx_hash).unwrap();
            assert_eq!(c.pref, tx_hash);

            origin_txs.push(tx_hash.clone());
            original_spent_amounts.push(amount);
        }

        // Try to spend non-conflicting cells several times and check that pref remains the same
        let mut iteration = 0;
        while iteration < 20 {
            if !original_spent_amounts.contains(&(iteration as Capacity)) {
                let n = thread_rng().gen_range(0, 3);
                let origin_tx_hash = *origin_txs.get(n).unwrap();

                // A transaction that spends the same inputs but produces a distinct output should conflict.
                let tx = Cell::new(
                    Inputs::new(vec![inputs[n].clone()]),
                    Outputs::new(vec![transfer::transfer_output(pkh2.clone(), iteration).unwrap()]),
                );
                dh.insert_cell(tx.clone()).unwrap();
                let c = dh.conflicting_cells(&tx.hash()).unwrap();
                assert_eq!(c.pref, origin_tx_hash); // pref must be the original one which succeeded last time
            }
            iteration += 1;
        }

        // Spend cells with an input having a valid non-conflicting cell
        let mut new_hash = origin_txs.get(0).unwrap().clone();
        let mut previous_hash = new_hash.clone();
        while iteration < 25 {
            // A transaction that spends a distinct input should not conflict.
            let tx = Cell::new(
                Inputs::new(vec![Input::new(&kp1, new_hash, 0).unwrap()]),
                Outputs::new(vec![transfer::transfer_output(pkh1.clone(), iteration).unwrap()]),
            );
            dh.insert_cell(tx.clone()).unwrap();
            let tx_hash = tx.hash();
            let conflict_cell = dh.conflicting_cells(&tx_hash).unwrap();
            // pref must be the one which was inserted recently without conflicts
            assert_eq!(conflict_cell.pref, tx_hash);

            previous_hash = new_hash.clone();
            new_hash = tx_hash;
            iteration += 1;
        }

        // Spend another round of cells, having input with the previous cell, which has been spent already,
        // and check that it conflicts with the latest successful spent cell.
        while iteration < 30 {
            let tx = Cell::new(
                Inputs::new(vec![Input::new(&kp1, previous_hash, 0).unwrap()]),
                Outputs::new(vec![transfer::transfer_output(pkh1.clone(), iteration).unwrap()]),
            );
            dh.insert_cell(tx.clone()).unwrap();
            let tx_hash = tx.hash();
            let conflict_cell = dh.conflicting_cells(&tx_hash).unwrap();
            assert_eq!(conflict_cell.pref, new_hash);
            iteration += 1;
        }
    }

    #[actix_rt::test]
    async fn test_conflict_graph() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        // Some root unspent outputs for `genesis`. We assume this input refers to a cell with funds
        // but for the purposes of the conflict graph it doesn't matter.
        let genesis_op = CoinbaseOperation::new(vec![
            (pkh1.clone(), 1000),
            (pkh2.clone(), 1000),
            (pkh2.clone(), 500),
        ]);
        let genesis_tx: Cell = genesis_op.try_into().unwrap();
        let genesis_output_cell_ids =
            CellIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs()).unwrap();

        let mut dh: ConflictGraph = ConflictGraph::new(genesis_output_cell_ids.clone());

        let input1 = Input::new(&kp1, genesis_tx.hash(), 0).unwrap();
        let input2 = Input::new(&kp2, genesis_tx.hash(), 1).unwrap();
        let input3 = Input::new(&kp2, genesis_tx.hash(), 2).unwrap();

        // A transaction that spends `genesis` and produces a new output for `pkh2`.
        let tx1 = Cell::new(
            Inputs::new(vec![input1.clone()]),
            Outputs::new(vec![transfer::transfer_output(pkh2.clone(), 900).unwrap()]),
        );
        dh.insert_cell(tx1.clone()).unwrap();
        let expected: HashSet<CellHash> = vec![tx1.hash()].iter().cloned().collect();
        let c1 = dh.conflicting_cells(&tx1.hash()).unwrap();
        assert_eq!(c1.conflicts.len(), 1);
        assert_eq!(c1.conflicts, expected);
        assert_eq!(c1.pref, tx1.hash());

        // A transaction that spends the same inputs but produces a distinct output should conflict.
        let tx2 = Cell::new(
            Inputs::new(vec![input1.clone()]),
            Outputs::new(vec![transfer::transfer_output(pkh2.clone(), 800).unwrap()]),
        );
        dh.insert_cell(tx2.clone()).unwrap();
        let expected: HashSet<CellHash> = vec![tx1.hash(), tx2.hash()].iter().cloned().collect();
        let c2 = dh.conflicting_cells(&tx2.hash()).unwrap();
        assert_eq!(c2.conflicts.len(), 2);
        assert_eq!(c2.conflicts, expected);
        assert_eq!(c2.pref, tx1.hash());

        // A transaction that spends a distinct input should not conflict.
        let tx3 = Cell::new(
            Inputs::new(vec![input2.clone(), input3.clone()]),
            Outputs::new(vec![transfer::transfer_output(pkh1.clone(), 700).unwrap()]),
        );
        dh.insert_cell(tx3.clone()).unwrap();
        let expected: HashSet<CellHash> = vec![tx3.hash()].iter().cloned().collect();
        let c3 = dh.conflicting_cells(&tx3.hash()).unwrap();
        assert_eq!(c3.conflicts.len(), 1);
        assert_eq!(c3.conflicts, expected);
        assert_eq!(c3.pref, tx3.hash());
    }

    #[actix_rt::test]
    async fn test_multiple_inputs() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        // Some root unspent outputs for `genesis`. We assume this input refers to a CELL with funds
        // but for the purposes of the hypergraph it doesn't matter.
        let genesis_op = CoinbaseOperation::new(vec![
            (pkh1.clone(), 1000),
            (pkh2.clone(), 1000),
            (pkh2.clone(), 500),
        ]);
        let genesis_tx: Cell = genesis_op.try_into().unwrap();
        let genesis_output_cell_ids =
            CellIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs()).unwrap();

        let mut dh: ConflictGraph = ConflictGraph::new(genesis_output_cell_ids.clone());

        let input1 = Input::new(&kp1, genesis_tx.hash(), 0).unwrap();
        let input2 = Input::new(&kp2, genesis_tx.hash(), 1).unwrap();
        let input3 = Input::new(&kp2, genesis_tx.hash(), 2).unwrap();

        // A transaction that spends `genesis` and produces a new output for `pkh2`.
        let output1 = transfer::transfer_output(pkh2, 1000).unwrap();
        let tx1 = Cell::new(Inputs::new(vec![input1.clone()]), Outputs::new(vec![output1.clone()]));
        dh.insert_cell(tx1.clone()).unwrap();
        let expected: HashSet<CellHash> = vec![tx1.hash()].iter().cloned().collect();
        let c1 = dh.conflicting_cells(&tx1.hash()).unwrap();
        assert_eq!(c1.conflicts.len(), 1);
        assert_eq!(c1.conflicts, expected);
        assert_eq!(c1.pref, tx1.hash());

        // A transaction that spends the same inputs but produces a distinct output should conflict.
        let output2 = transfer::transfer_output(pkh2, 900).unwrap();
        let tx2 = Cell::new(
            Inputs::new(vec![input1.clone(), input2.clone()]),
            Outputs::new(vec![output2.clone()]),
        );
        dh.insert_cell(tx2.clone()).unwrap();
        let expected: HashSet<CellHash> = vec![tx1.hash(), tx2.hash()].iter().cloned().collect();
        let c2 = dh.conflicting_cells(&tx2.hash()).unwrap();
        assert_eq!(c2.conflicts.len(), 2);
        assert_eq!(c2.conflicts, expected);
        assert_eq!(c2.pref, tx1.hash());

        // A transaction that spends a distinct input should not conflict.
        let tx3 = Cell::new(Inputs::new(vec![input3.clone()]), Outputs::new(vec![output2.clone()]));
        dh.insert_cell(tx3.clone()).unwrap();
        let expected: HashSet<CellHash> = vec![tx3.hash()].iter().cloned().collect();
        let c3 = dh.conflicting_cells(&tx3.hash()).unwrap();
        assert_eq!(c3.conflicts.len(), 1);
        assert_eq!(c3.conflicts, expected);
        assert_eq!(c3.pref, tx3.hash());

        // A transaction that spends multiple conflicting inputs
        let output3 = transfer::transfer_output(pkh2, 800).unwrap();
        let tx4 = Cell::new(
            Inputs::new(vec![input1.clone(), input2.clone(), input3.clone()]),
            Outputs::new(vec![output3]),
        );
        dh.insert_cell(tx4.clone()).unwrap();
        let expected: HashSet<CellHash> =
            vec![tx1.hash(), tx2.hash(), tx3.hash(), tx4.hash()].iter().cloned().collect();
        let c4 = dh.conflicting_cells(&tx4.hash()).unwrap();
        assert_eq!(c4.conflicts.len(), 4);
        assert_eq!(c4.conflicts, expected);
        assert_eq!(c4.pref, tx1.hash());
    }

    #[actix_rt::test]
    async fn test_accept_cell() {
        let (kp1, _kp2, pkh1, pkh2) = generate_keys();

        // Some root unspent outputs for `genesis`. We assume this input refers to a cell with funds
        // but for the purposes of the conflict graph it doesn't matter.
        let genesis_op = CoinbaseOperation::new(vec![(pkh1.clone(), 1000), (pkh2.clone(), 1000)]);
        let genesis_tx: Cell = genesis_op.try_into().unwrap();
        let genesis_output_cell_ids =
            CellIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs()).unwrap();

        let mut dh: ConflictGraph = ConflictGraph::new(genesis_output_cell_ids.clone());

        let input1 = Input::new(&kp1, genesis_tx.hash(), 0).unwrap();

        // A transaction that spends `genesis` and produces a new output for `pkh2`.
        let tx1 = Cell::new(
            Inputs::new(vec![input1.clone()]),
            Outputs::new(vec![transfer::transfer_output(pkh2.clone(), 900).unwrap()]),
        );
        dh.insert_cell(tx1.clone()).unwrap();
        let expected: HashSet<CellHash> = vec![tx1.hash()].iter().cloned().collect();
        let c1 = dh.conflicting_cells(&tx1.hash()).unwrap();
        assert_eq!(c1.conflicts.len(), 1);
        assert_eq!(c1.conflicts, expected);
        assert_eq!(c1.pref, tx1.hash());

        // A transaction that spends the same inputs but produces a distinct output should conflict.
        let tx2 = Cell::new(
            Inputs::new(vec![input1.clone()]),
            Outputs::new(vec![transfer::transfer_output(pkh2.clone(), 800).unwrap()]),
        );
        dh.insert_cell(tx2.clone()).unwrap();
        let expected: HashSet<CellHash> = vec![tx1.hash(), tx2.hash()].iter().cloned().collect();
        let c2 = dh.conflicting_cells(&tx2.hash()).unwrap();
        assert_eq!(c2.conflicts.len(), 2);
        assert_eq!(c2.conflicts, expected);
        assert_eq!(c2.pref, tx1.hash());

        let conflicts_removed = dh.accept_cell(tx2.clone()).unwrap();
        let expected = vec![tx1.hash()];
        assert_eq!(conflicts_removed, expected);
    }

    #[actix_rt::test]
    async fn test_accept_cell2() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        // Some root unspent outputs for `genesis`. We assume this input refers to a CELL with funds
        // but for the purposes of the hypergraph it doesn't matter.
        let genesis_op = CoinbaseOperation::new(vec![
            (pkh1.clone(), 1000),
            (pkh2.clone(), 1000),
            (pkh2.clone(), 500),
        ]);
        let genesis_tx: Cell = genesis_op.try_into().unwrap();
        let genesis_output_cell_ids =
            CellIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs()).unwrap();

        let mut dh: ConflictGraph = ConflictGraph::new(genesis_output_cell_ids.clone());

        let input1 = Input::new(&kp1, genesis_tx.hash(), 0).unwrap();
        let input2 = Input::new(&kp2, genesis_tx.hash(), 1).unwrap();
        let input3 = Input::new(&kp2, genesis_tx.hash(), 2).unwrap();

        // A transaction that spends `genesis` and produces a new output for `pkh2`.
        let output1 = transfer::transfer_output(pkh2, 1000).unwrap();
        let tx1 = Cell::new(Inputs::new(vec![input1.clone()]), Outputs::new(vec![output1.clone()]));
        dh.insert_cell(tx1.clone()).unwrap();
        let expected: HashSet<CellHash> = vec![tx1.hash()].iter().cloned().collect();
        let c1 = dh.conflicting_cells(&tx1.hash()).unwrap();
        assert_eq!(c1.conflicts.len(), 1);
        assert_eq!(c1.conflicts, expected);
        assert_eq!(c1.pref, tx1.hash());

        // A transaction that spends the same inputs but produces a distinct output should conflict.
        let output2 = transfer::transfer_output(pkh2, 900).unwrap();
        let tx2 = Cell::new(
            Inputs::new(vec![input1.clone(), input2.clone()]),
            Outputs::new(vec![output2.clone()]),
        );
        dh.insert_cell(tx2.clone()).unwrap();
        let expected: HashSet<CellHash> = vec![tx1.hash(), tx2.hash()].iter().cloned().collect();
        let c2 = dh.conflicting_cells(&tx2.hash()).unwrap();
        assert_eq!(c2.conflicts.len(), 2);
        assert_eq!(c2.conflicts, expected);
        assert_eq!(c2.pref, tx1.hash());

        // A transaction that spends a distinct input should not conflict.
        let tx3 = Cell::new(Inputs::new(vec![input3.clone()]), Outputs::new(vec![output2.clone()]));
        dh.insert_cell(tx3.clone()).unwrap();
        let expected: HashSet<CellHash> = vec![tx3.hash()].iter().cloned().collect();
        let c3 = dh.conflicting_cells(&tx3.hash()).unwrap();
        assert_eq!(c3.conflicts.len(), 1);
        assert_eq!(c3.conflicts, expected);
        assert_eq!(c3.pref, tx3.hash());

        // A transaction that spends multiple conflicting inputs
        let output3 = transfer::transfer_output(pkh2, 800).unwrap();
        let tx4 = Cell::new(
            Inputs::new(vec![input1.clone(), input2.clone(), input3.clone()]),
            Outputs::new(vec![output3]),
        );
        dh.insert_cell(tx4.clone()).unwrap();
        let expected: HashSet<CellHash> =
            vec![tx1.hash(), tx2.hash(), tx3.hash(), tx4.hash()].iter().cloned().collect();
        let c4 = dh.conflicting_cells(&tx4.hash()).unwrap();
        assert_eq!(c4.conflicts.len(), 4);
        assert_eq!(c4.conflicts, expected);
        assert_eq!(c4.pref, tx1.hash());

        let mut conflicts_removed = dh.accept_cell(tx4.clone()).unwrap();
        conflicts_removed.sort();
        let mut expected = vec![tx1.hash(), tx2.hash(), tx3.hash()];
        expected.sort();
        assert_eq!(conflicts_removed, expected);
    }

    #[actix_rt::test]
    async fn test_disjoint_inputs() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        // Some root unspent outputs for `genesis`. We assume this input refers to a CELL with funds
        // but for the purposes of the hypergraph it doesn't matter.
        let genesis_op = CoinbaseOperation::new(vec![
            (pkh1.clone(), 1000),
            (pkh2.clone(), 1000),
            (pkh2.clone(), 500),
            (pkh2.clone(), 400),
        ]);
        let genesis_tx: Cell = genesis_op.try_into().unwrap();
        let genesis_output_cell_ids =
            CellIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs()).unwrap();

        let mut dh: ConflictGraph = ConflictGraph::new(genesis_output_cell_ids.clone());

        let input1 = Input::new(&kp1, genesis_tx.hash(), 0).unwrap();
        let input2 = Input::new(&kp2, genesis_tx.hash(), 1).unwrap();
        let input3 = Input::new(&kp2, genesis_tx.hash(), 2).unwrap();
        let input4 = Input::new(&kp2, genesis_tx.hash(), 3).unwrap();

        // A transaction that spends `genesis` and produces a new output for `pkh2`.
        let output1 = transfer::transfer_output(pkh2, 1000).unwrap();
        let tx1 = Cell::new(Inputs::new(vec![input1.clone()]), Outputs::new(vec![output1.clone()]));
        dh.insert_cell(tx1.clone()).unwrap();
        let expected: HashSet<CellHash> = vec![tx1.hash()].iter().cloned().collect();
        let c1 = dh.conflicting_cells(&tx1.hash()).unwrap();
        assert_eq!(c1.conflicts.len(), 1);
        assert_eq!(c1.conflicts, expected);
        assert_eq!(c1.pref, tx1.hash());

        // A transaction that spends some of the same inputs as `tx1`
        let output2 = transfer::transfer_output(pkh2, 900).unwrap();
        let tx2 = Cell::new(
            Inputs::new(vec![input1.clone(), input2.clone()]),
            Outputs::new(vec![output2.clone()]),
        );
        dh.insert_cell(tx2.clone()).unwrap();
        let expected: HashSet<CellHash> = vec![tx1.hash(), tx2.hash()].iter().cloned().collect();
        let c2 = dh.conflicting_cells(&tx2.hash()).unwrap();
        assert_eq!(c2.conflicts.len(), 2);
        assert_eq!(c2.conflicts, expected);
        assert_eq!(c2.pref, tx1.hash());

        // A transaction that spends some of the same inputs as `tx2`
        let output3 = transfer::transfer_output(pkh2, 800).unwrap();
        let tx3 = Cell::new(
            Inputs::new(vec![input2.clone(), input3.clone(), input4.clone()]),
            Outputs::new(vec![output3.clone()]),
        );
        dh.insert_cell(tx3.clone()).unwrap();
        let expected: HashSet<CellHash> = vec![tx2.hash(), tx3.hash()].iter().cloned().collect();
        let c3 = dh.conflicting_cells(&tx3.hash()).unwrap();
        assert_eq!(c3.conflicts.len(), 2);
        assert_eq!(c3.conflicts, expected);
        assert_eq!(c3.pref, tx1.hash());

        // A transaction that spends one of the same inputs as `tx3`
        let output4 = transfer::transfer_output(pkh2, 700).unwrap();
        let tx4 = Cell::new(Inputs::new(vec![input3.clone()]), Outputs::new(vec![output4.clone()]));
        dh.insert_cell(tx4.clone()).unwrap();
        let expected: HashSet<CellHash> = vec![tx3.hash(), tx4.hash()].iter().cloned().collect();
        let c4 = dh.conflicting_cells(&tx4.hash()).unwrap();
        assert_eq!(c4.conflicts.len(), 2);
        assert_eq!(c4.conflicts, expected);
        assert_eq!(c4.pref, tx1.hash());

        // Another transaction that spends one of the same inputs as `tx3`
        let output5 = transfer::transfer_output(pkh2, 600).unwrap();
        let tx5 = Cell::new(Inputs::new(vec![input4.clone()]), Outputs::new(vec![output5.clone()]));
        dh.insert_cell(tx5.clone()).unwrap();
        let expected: HashSet<CellHash> = vec![tx3.hash(), tx5.hash()].iter().cloned().collect();
        let c5 = dh.conflicting_cells(&tx5.hash()).unwrap();
        assert_eq!(c5.conflicts.len(), 2);
        assert_eq!(c5.conflicts, expected);
        assert_eq!(c5.pref, tx1.hash());
    }

    #[actix_rt::test]
    async fn test_outputs() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        // Some root unspent outputs for `genesis`. We assume this input refers to a CELL with funds
        // but for the purposes of the hypergraph it doesn't matter.
        let genesis_op = CoinbaseOperation::new(vec![
            (pkh1.clone(), 1000),
            (pkh2.clone(), 1000),
            (pkh2.clone(), 500),
        ]);
        let genesis_tx: Cell = genesis_op.try_into().unwrap();
        let genesis_output_cell_ids =
            CellIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs()).unwrap();

        let mut dh: ConflictGraph = ConflictGraph::new(genesis_output_cell_ids.clone());

        let input1 = Input::new(&kp1, genesis_tx.hash(), 0).unwrap();

        // A transaction that spends `genesis` and produces two new outputs
        let tx1 = Cell::new(
            Inputs::new(vec![input1.clone()]),
            Outputs::new(vec![
                transfer::transfer_output(pkh1, 1000).unwrap(),
                transfer::transfer_output(pkh2, 1000).unwrap(),
            ]),
        );
        dh.insert_cell(tx1.clone()).unwrap();
        let c1 = dh.conflicting_cells(&tx1.hash()).unwrap();
        let expected: HashSet<CellHash> = vec![tx1.hash()].iter().cloned().collect();
        assert_eq!(c1.conflicts.len(), 1);
        assert_eq!(c1.conflicts, expected);

        // A transaction that spends one output from `tx1` and produces a new output.
        let input2 = Input::new(&kp1, tx1.hash(), 0).unwrap();
        let tx2 = Cell::new(
            Inputs::new(vec![input2.clone()]),
            Outputs::new(vec![transfer::transfer_output(pkh1, 1000).unwrap()]),
        );
        dh.insert_cell(tx2.clone()).unwrap();
        let c2 = dh.conflicting_cells(&tx2.hash()).unwrap();
        let expected: HashSet<CellHash> = vec![tx2.hash()].iter().cloned().collect();
        assert_eq!(c2.conflicts.len(), 1);
        assert_eq!(c2.conflicts, expected);

        // A transaction that spends another output from `tx1` and produces two new outputs.
        let input3 = Input::new(&kp2, tx1.hash(), 1).unwrap();
        let tx3 = Cell::new(
            Inputs::new(vec![input3.clone()]),
            Outputs::new(vec![
                transfer::transfer_output(pkh1, 1000).unwrap(),
                transfer::transfer_output(pkh2, 1000).unwrap(),
            ]),
        );
        dh.insert_cell(tx3.clone()).unwrap();
        let c3 = dh.conflicting_cells(&tx3.hash()).unwrap();
        let expected: HashSet<CellHash> = vec![tx3.hash()].iter().cloned().collect();
        assert_eq!(c3.conflicts.len(), 1);
        assert_eq!(c3.conflicts, expected);

        // A transaction which spends tx3 outputs and conflicts with tx1
        let input4 = Input::new(&kp1, tx3.hash(), 0).unwrap();
        let tx4 = Cell::new(
            Inputs::new(vec![input1.clone(), input4.clone()]),
            Outputs::new(vec![transfer::transfer_output(pkh1, 1000).unwrap()]),
        );
        dh.insert_cell(tx4.clone()).unwrap();
        let c4 = dh.conflicting_cells(&tx4.hash()).unwrap();
        let expected: HashSet<CellHash> = vec![tx1.hash(), tx4.hash()].iter().cloned().collect();
        assert_eq!(c4.conflicts.len(), 2);
        assert_eq!(c4.conflicts, expected);
        assert_eq!(c4.pref, tx1.hash());
    }

    fn hash_public(keypair: &Keypair) -> [u8; 32] {
        let enc = bincode::serialize(&keypair.public).unwrap();
        blake3::hash(&enc).as_bytes().clone()
    }

    fn generate_keys() -> (Keypair, Keypair, [u8; 32], [u8; 32]) {
        let kp1_hex = "ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416".to_owned();
        let kp2_hex = "5a353c630d3faf8e2d333a0983c1c71d5e9b6aed8f4959578fbeb3d3f3172886393b576de0ac1fe86a4dd416cf032543ac1bd066eb82585f779f6ce21237c0cd".to_owned();

        let kp1 = Keypair::from_bytes(&hex::decode(kp1_hex).unwrap()).unwrap();
        let kp2 = Keypair::from_bytes(&hex::decode(kp2_hex).unwrap()).unwrap();

        let pkh1 = hash_public(&kp1);
        let pkh2 = hash_public(&kp2);

        return (kp1, kp2, pkh1, pkh2);
    }
}

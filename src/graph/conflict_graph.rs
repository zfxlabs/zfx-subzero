use super::{Error, Result};

use crate::cell::inputs::{Input, Inputs};
use crate::cell::types::CellHash;
use crate::cell::{Cell, CellIds};

use crate::sleet::conflict_set::ConflictSet;
use crate::sleet::BETA2;

use std::collections::{hash_map::Entry, HashMap, HashSet};

pub struct ConflictGraph {
    dh: HashMap<CellIds, Vec<Cell>>,
    // Maintains an ordered set of transactions for conflict set preference.
    cs: HashMap<CellHash, ConflictSet<CellHash>>,
}

impl ConflictGraph {
    pub fn new(genesis: CellIds) -> Self {
        let mut adj = HashMap::default();
        adj.insert(genesis, vec![]);
        ConflictGraph {
            dh: adj,
            // Note: genesis cannot conflict.
            // cs: vec![],
            cs: HashMap::new(),
        }
    }

    pub fn insert_cell(&mut self, cell: Cell) -> Result<()> {
        // The cell ids that this transaction consumes.
        let mut consumed_cell_ids = CellIds::from_inputs(cell.inputs())?;
        // If there exists an intersecting set of cell ids in the hypergraph then we are
        // spending from those outputs.
        let mut intersecting_vertices = HashSet::new();
        for (cell_ids, _) in self.dh.iter() {
            if consumed_cell_ids.intersects_with(cell_ids) {
                intersecting_vertices.insert(cell_ids.clone());
                // Remove the intersecting cell ids.
                consumed_cell_ids = consumed_cell_ids.left_difference(cell_ids);
            }
        }
        // If we did not succeed in finding all the cell ids being consumed then this is
        // an error - an entry must exist in order to be spent.
        if consumed_cell_ids.len() > 0 {
            return Err(Error::UndefinedCell);
        } else {
            let cell_hash = cell.hash();
            let produced_cell_ids = CellIds::from_outputs(cell_hash, cell.outputs())?;
            // First we make sure that the produced `cell_ids` exist within the hypergraph.
            match self.dh.entry(produced_cell_ids.clone()) {
                // If the produced cell ids already exist then we have an error - a duplicate
                // transaction exists in the hypergraph. This implies that the transaction
                // had the same hash as another transaction through the hash in `from_outputs`.
                Entry::Occupied(_) => return Err(Error::DuplicateCell),
                // Otherwise we create an empty entry (same as when creating genesis).
                Entry::Vacant(mut v) => {
                    let _ = v.insert(vec![]);
                }
            }

            if self.cs.contains_key(&cell_hash) {
                return Err(Error::DuplicateCell);
            }
            self.cs.insert(cell_hash, ConflictSet::new(cell_hash));
            // let now4 = std::time::Instant::now();

            // For each set of intersecting vertices (CellId bundles) an arc is produced relating the
            // CellIds to the new transaction.
            let consumed_cell_ids = CellIds::from_inputs(cell.inputs())?;
            let mut conflicts = vec![];
            for cell_ids in intersecting_vertices.iter() {
                match self.dh.entry(cell_ids.clone()) {
                    Entry::Occupied(mut o) => {
                        let arcs = o.get_mut();

                        // Save existing conflicting transactions.
                        for arc_cell in arcs.iter() {
                            if *arc_cell == cell {
                                return Err(Error::DuplicateCell);
                            }
                            if !arc_cell.inputs().is_disjoint(&cell.inputs()) {
                                conflicts.push(arc_cell.hash());
                            }
                        }
                        arcs.push(cell.clone());
                    }
                    Entry::Vacant(_) => return Err(Error::UndefinedCell),
                }
            }

            // For all the transactions that we conflict with, we wish to add the conflicts to the
            // conflicts sets of this transaction and any conflicting transactions, whilst saving
            // the transactions by order of preference - this is determined by insertion order.
            let mut ordered_conflicting_cells = vec![];
            let mut pref = None;
            let mut last = None;
            let mut cnt = 0u8;
            for conflicting_cell_hash in conflicts.iter_mut() {
                // Note: We do not change the properties of the conflict set since this one
                // came first and is thus preferred.
                match self.cs.get_mut(conflicting_cell_hash) {
                    Some(set) => {
                        set.conflicts.insert(cell_hash.clone());
                        // Save the properties of the first conflict set (the most preferred).
                        if pref.is_none() {
                            pref = Some(set.pref.clone());
                            last = Some(set.last.clone());
                            cnt = set.cnt;
                        }
                        ordered_conflicting_cells.push(*conflicting_cell_hash);
                    }
                    None => {}
                }
            }
            // Update the conflict set of this transaction based on the ordered cells.
            if ordered_conflicting_cells.len() > 0 {
                if let Some(set) = self.cs.get_mut(&cell_hash) {
                    set.pref = pref.unwrap();
                    // FIXME: Not sure here.
                    set.last = last.unwrap();
                    set.cnt = cnt;
                    for conflicting_cell_hash in ordered_conflicting_cells.iter() {
                        set.conflicts.insert(*conflicting_cell_hash);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn accept_cell(&mut self, cell: Cell) -> Result<Vec<CellHash>> {
        // Once a transaction is accepted we wish to remove all the conflicts from the graph
        // in order to free up space for future entries.
        // The conflicting cells are returned in order to allow Sleet to make
        // the necessary adjustment to other data structures
        let mut conflicting_hashes: HashSet<CellHash> = HashSet::new();
        match self.conflicting_cells(&cell.hash()) {
            Some(conflict_set) => {
                // If the transaction does not conflict then we are done.
                if conflict_set.is_singleton() {
                    return Ok(vec![]);
                }

                // First fetch all the conflicting cell ids produced by the conflicting cells,
                // excluding the `cell` being accepted.
                // TODO: check why duplicates are possible here!
                let mut conflicting_cell_ids = HashSet::new();
                for conflicting_cell_hash in conflict_set.conflicts.iter() {
                    if cell.hash().eq(conflicting_cell_hash) {
                        continue;
                    }
                    let cell_ids = CellIds::from_outputs(cell.hash(), cell.outputs())?;
                    let _ = conflicting_cell_ids.insert(cell_ids);
                    let _ = conflicting_hashes.insert(conflicting_cell_hash.clone());
                }

                // Next remove each vertex from the graph which is a conflicting `cell_id`.

                for conflicting_cell_id in conflicting_cell_ids.iter() {
                    self.dh.remove(&conflicting_cell_id).unwrap();
                }

                // Next remove each arc which point to the conflicting transactions (which no
                // longer exist).
                for (_, arcs) in self.dh.iter_mut() {
                    arcs.retain(|arc| {
                        arc.clone() == cell.clone() || !conflict_set.conflicts.contains(&arc.hash())
                    });
                }

                // Next remove the conflicting transactions from the conflict sets, preserving
                // the ordering.
                let cell_hash = cell.hash();
                self.cs.insert(cell_hash, ConflictSet::new(cell_hash));
                conflict_set.conflicts.iter()
                    .for_each(|cs| {
                        self.cs.remove(cs);
                    });
                Ok(conflicting_hashes.iter().cloned().collect())
            }
            // If the transaction has no conflict set then it is invalid.
            None => Err(Error::UndefinedCell),
        }
    }

    pub fn conflicting_cells(&self, cell_hash: &CellHash) -> Option<ConflictSet<CellHash>> {
        self.cs.get(cell_hash).cloned()
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
                None => { Err(Error::UndefinedCellHash(cell_hash.clone()))}
            }
        } else {
            Err(Error::EmptyConflictGraph)
        }
    }
}

#[cfg(test)]
mod test {
    use super::ConflictGraph;

    use crate::alpha::coinbase::CoinbaseOperation;
    use crate::alpha::transfer;

    use crate::cell::inputs::{Input, Inputs};
    use crate::cell::outputs::{Output, Outputs};
    use crate::cell::types::CellHash;
    use crate::cell::{Cell, CellIds};

    use crate::sleet::conflict_set::ConflictSet;

    use std::collections::HashSet;
    use std::convert::TryInto;

    use ed25519_dalek::Keypair;

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
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        // Some root unspent outputs for `genesis`. We assume this input refers to a cell with funds
        // but for the purposes of the conflict graph it doesn't matter.
        let genesis_op = CoinbaseOperation::new(vec![(pkh1.clone(), 1000), (pkh2.clone(), 1000)]);
        let genesis_tx: Cell = genesis_op.try_into().unwrap();
        let genesis_output_cell_ids =
            CellIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs()).unwrap();

        let mut dh: ConflictGraph = ConflictGraph::new(genesis_output_cell_ids.clone());

        let input1 = Input::new(&kp1, genesis_tx.hash(), 0).unwrap();
        let input2 = Input::new(&kp2, genesis_tx.hash(), 1).unwrap();

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

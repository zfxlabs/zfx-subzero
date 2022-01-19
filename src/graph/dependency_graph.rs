use super::{Error, Result};
use crate::cell::{Cell, CellIds};

use std::collections::{hash_map::Entry, HashMap, VecDeque};

/// The dependency graph is a cell graph which maps produced outputs to consumed inputs in cells.
/// The cell graphs purpose is to order cells by their dependencies.
pub struct DependencyGraph {
    /// An adjacency list from produced output cell ids to consumed input cell ids
    dh: HashMap<CellIds, CellIds>,
    /// Vertices in the graph with no inbound edges.
    roots: Vec<CellIds>,
}

/// Returns true if some produced cell ids intersect (have an existing edge directed).
pub fn has_inbound_edges(dh: &HashMap<CellIds, CellIds>, produced_cell_ids: &CellIds) -> bool {
    for (_, consumed_cell_ids) in dh.iter() {
        if produced_cell_ids.intersects_with(consumed_cell_ids) {
            return true;
        }
    }
    false
}

impl DependencyGraph {
    pub fn new() -> Self {
        DependencyGraph { dh: HashMap::new(), roots: vec![] }
    }

    pub fn insert(&mut self, cell: Cell) -> Result<()> {
        let produced_cell_ids = CellIds::from_outputs(cell.hash(), cell.outputs())?;
        let consumed_cell_ids = CellIds::from_inputs(cell.inputs())?;
        match self.dh.entry(produced_cell_ids.clone()) {
            Entry::Occupied(_) => return Err(Error::DuplicateCell),
            Entry::Vacant(mut v) => {
                let _ = v.insert(consumed_cell_ids.clone());
            }
        }

        // If the consumed cell ids do not intersect with existing produced cell ids in the roots,
        // keep them as roots, otherwise remove the root.
        let mut roots = vec![];
        for root_cell_ids in self.roots.iter() {
            if !consumed_cell_ids.intersects_with(root_cell_ids) {
                roots.push(root_cell_ids.clone());
            }
        }
        self.roots = roots;

        // If the produced cell ids by this cell are referenced by existing consumers, do not store
        // this vertex as a root. Otherwise store it as a potential root vertex.
        let mut referenced_by_existing_consumers = false;
        for (_, referenced_cell_ids) in self.dh.iter() {
            if produced_cell_ids.intersects_with(referenced_cell_ids) {
                referenced_by_existing_consumers = true;
                break;
            }
        }
        if !referenced_by_existing_consumers {
            self.roots.push(produced_cell_ids.clone());
        }
        Ok(())
    }

    pub fn topological(&self) -> Result<Vec<CellIds>> {
        // Empty list that contains the sorted elements.
        let mut sorted = VecDeque::new();
        let mut roots = self.roots.clone();
        let mut graph = self.dh.clone();
        loop {
            if roots.len() == 0 {
                break;
            }
            let root = roots.pop().unwrap();
            sorted.push_front(root.clone());

            // Remove edges poining from a root to other vertices
            let mut removed_edges = CellIds::empty();
            match graph.entry(root.clone()) {
                Entry::Occupied(mut o) => {
                    let edges = o.get_mut();
                    removed_edges = edges.clone();
                    *edges = CellIds::empty();
                }
                Entry::Vacant(v) => return Err(Error::UndefinedCell),
            };

            // Find all of the producers which intersect with the consumers
            for (producer, _) in graph.iter() {
                if producer.intersects_with(&removed_edges) {
                    if !has_inbound_edges(&graph, producer) {
                        roots.push(producer.clone());
                    }
                }
            }
        }
        Ok(sorted.iter().cloned().collect())
    }

    pub fn topological_cells(&self, cells: Vec<Cell>) -> Result<Vec<Cell>> {
        let sorted_cell_ids = self.topological()?;
        let mut sorted_cells = vec![];
        for cell_ids in sorted_cell_ids.iter() {
            for cell in cells.iter() {
                let output_cell_ids = CellIds::from_outputs(cell.hash(), cell.outputs())?;
                if output_cell_ids.eq(cell_ids) {
                    sorted_cells.push(cell.clone());
                }
            }
        }
        Ok(sorted_cells)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::alpha::coinbase::CoinbaseOperation;
    use crate::alpha::transfer::TransferOperation;

    use std::convert::TryInto;

    use ed25519_dalek::Keypair;

    #[actix_rt::test]
    async fn test_dependency_graph() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        let mut g = DependencyGraph::new();

        let genesis_op = CoinbaseOperation::new(vec![(pkh1.clone(), 1000), (pkh1.clone(), 1000)]);
        // This cell is a root and does not depend on any other cell
        let genesis_tx: Cell = genesis_op.try_into().unwrap();
        let genesis_tx_output_cell_ids =
            CellIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs()).unwrap();

        // This cell depends on genesis
        let op1 = TransferOperation::new(genesis_tx.clone(), pkh1.clone(), pkh1.clone(), 1000);
        let tx1 = op1.transfer(&kp1).unwrap();
        let tx1_cell_ids = CellIds::from_outputs(tx1.hash(), tx1.outputs()).unwrap();

        // This cell depends on tx1
        let op2 = TransferOperation::new(tx1.clone(), pkh1.clone(), pkh1.clone(), 900);
        let tx2 = op2.transfer(&kp1).unwrap();
        let tx2_cell_ids = CellIds::from_outputs(tx2.hash(), tx2.outputs()).unwrap();

        // This cell depends on tx1
        let op3 = TransferOperation::new(tx1.clone(), pkh1.clone(), pkh1.clone(), 800);
        let tx3 = op3.transfer(&kp1).unwrap();
        let tx3_cell_ids = CellIds::from_outputs(tx3.hash(), tx3.outputs()).unwrap();

        // This cell depends on tx2
        let op4 = TransferOperation::new(tx2.clone(), pkh1.clone(), pkh1.clone(), 700);
        let tx4 = op4.transfer(&kp1).unwrap();
        let tx4_cell_ids = CellIds::from_outputs(tx4.hash(), tx4.outputs()).unwrap();

        // This cell depends on tx3
        let op5 = TransferOperation::new(tx3.clone(), pkh1.clone(), pkh1.clone(), 600);
        let tx5 = op5.transfer(&kp1).unwrap();
        let tx5_cell_ids = CellIds::from_outputs(tx5.hash(), tx5.outputs()).unwrap();

        // Insert the transactions in some random order
        g.insert(tx4.clone());
        g.insert(tx2.clone());
        g.insert(genesis_tx.clone());
        g.insert(tx1.clone());
        g.insert(tx3.clone());
        g.insert(tx5.clone());

        assert_eq!(g.roots.len(), 2);
        assert_eq!(g.roots.clone(), vec![tx4_cell_ids.clone(), tx5_cell_ids.clone()]);
        assert_eq!(
            g.topological().unwrap(),
            vec![
                genesis_tx_output_cell_ids.clone(),
                tx1_cell_ids.clone(),
                tx2_cell_ids.clone(),
                // Note its okay for tx4 to come before tx3 since it depends on tx2, even though tx3
                // depends upon `tx1`.
                tx4_cell_ids.clone(),
                tx3_cell_ids.clone(),
                tx5_cell_ids.clone(),
            ]
        );
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

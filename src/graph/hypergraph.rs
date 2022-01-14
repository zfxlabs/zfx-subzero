use super::hyperarc::Hyperarc;
use super::{Error, Result};

use crate::chain::alpha::tx::{Input, Inputs, Output, Outputs, Tx};

use std::collections::{hash_map::Entry, HashMap, HashSet};

#[derive(Debug)]
pub struct Hypergraph {
    /// The adjacency lists of `H`. Each edge `I` can point to more than one vertex.
    h: HashMap<Outputs<Output>, Hyperarc>,
}

impl std::ops::Deref for Hypergraph {
    type Target = HashMap<Outputs<Output>, Hyperarc>;

    fn deref(&self) -> &'_ Self::Target {
        &self.h
    }
}

impl std::ops::DerefMut for Hypergraph {
    fn deref_mut(&mut self) -> &'_ mut Self::Target {
        &mut self.h
    }
}

impl Hypergraph {
    pub fn new(g: Outputs<Output>) -> Self {
        let mut map: HashMap<Outputs<Output>, Hyperarc> = HashMap::default();
        let _ = map.insert(g.clone(), Hyperarc::new());
        Hypergraph { h: map }
    }

    pub fn insert_tx(&mut self, spent_outputs: Outputs<Output>, tx: Tx) -> Result<()> {
        // Try to insert the new outputs.
        match self.entry(tx.outputs.clone()) {
            // If the new outputs already exists then it is a duplicate transaction.
            Entry::Occupied(_) => (),
            Entry::Vacant(v) => {
                // Insert an empty set of edges for the new output.
                let _ = v.insert(Hyperarc::new());
            }
        };
        // Update the input edges.
        match self.entry(spent_outputs.clone()) {
            Entry::Occupied(mut o) => {
                let hyperarc = o.get_mut();
                match hyperarc.get(&tx.inputs) {
                    // If there is already an equivalent inputs edge for the output being spent, then
                    // there is a conflict.
                    Some(existing) => {
                        let () = hyperarc.update(tx);
                    }
                    None => {
                        let _ = hyperarc.insert_new(tx).unwrap();
                    }
                }
            }
            // If the outputs being spent do not exist then error.
            Entry::Vacant(mut v) => return Err(Error::UndefinedUTXO),
        }
        Ok(())
    }

    pub fn conflicts(
        &self,
        spent_outputs: Outputs<Output>,
        inputs: Inputs<Input>,
    ) -> (Vec<Tx>, Tx) {
        let hyperarc = self.get(&spent_outputs).unwrap();
        let entry = hyperarc.get(&inputs).unwrap();
        let r: HashSet<Tx> = entry.0.iter().cloned().collect();
        let mut v: Vec<Tx> = r.iter().cloned().collect();
        v.sort();
        (v, entry.1.clone())
    }
}

// #[cfg(test)]
// mod test {
//     use super::Hypergraph;

//     use crate::chain::alpha::tx::{Tx, Inputs, Outputs};

//     use std::collections::HashSet;

//     #[actix_rt::test]
//     async fn test_failing() {
// 	let i1 = Inputs::new(vec![2, 3, 0, 1]);
// 	let i2 = Inputs::new(vec![3, 2]);
// 	let i3 = Inputs::new(vec![0, 1, 2, 3]);
// 	let i4 = Inputs::new(vec![2, 3]);
// 	assert_eq!(i1.cmp(&i2), std::cmp::Ordering::Less);
// 	assert_eq!(i3.cmp(&i4), std::cmp::Ordering::Less);

// 	let tx1 = Tx::new(vec![1, 3, 2, 0], vec![4]);
// 	let tx2 = Tx::new(vec![3, 2], vec![3]);
// 	let tx3 = Tx::new(vec![0, 1, 2, 3], vec![4]);
// 	assert_eq!(tx1.cmp(&tx2), std::cmp::Ordering::Less);
// 	assert_eq!(tx2.cmp(&tx3), std::cmp::Ordering::Greater);
//     }

//     #[actix_rt::test]
//     async fn test_hypergraph() {
// 	// Some root unspent output `go`.
// 	let go = Outputs::new(vec![0]);
// 	let mut hg: Hypergraph<u8, u8> = Hypergraph::new(go.clone());

// 	// A transaction that spends `go` with an input `gi`.
// 	let tx1 = Tx::new(vec![0], vec![1]);
// 	hg.insert_tx(go.clone(), tx1.clone()).unwrap();
// 	assert_eq!(hg.conflicts(go.clone(), tx1.inputs.clone()), (vec![tx1.clone()], tx1.clone()));

// 	// A transaction that spends the same input.
// 	let tx2 = Tx::new(vec![0], vec![2]);
// 	hg.insert_tx(go.clone(), tx2.clone()).unwrap();
// 	assert_eq!(hg.conflicts(go.clone(), tx2.inputs.clone()), (vec![tx1.clone(), tx2.clone()], tx1.clone()));

// 	// A transaction that spends a distinct input.
// 	let tx3 = Tx::new(vec![1], vec![3]);
// 	hg.insert_tx(go.clone(), tx3.clone()).unwrap();
// 	assert_eq!(hg.conflicts(go.clone(), tx3.inputs.clone()), (vec![tx3.clone()], tx3.clone()));
//     }

//     #[actix_rt::test]
//     async fn test_multiple_inputs() {
// 	// The genesis spendable outputs `go`
// 	let go = Outputs::new(vec![0]);
// 	let mut hg: Hypergraph<u8, u8> = Hypergraph::new(go.clone());

// 	// A transaction that spends `go` with an input `gi`.
// 	let tx1 = Tx::new(vec![0, 1], vec![1]);
// 	hg.insert_tx(go.clone(), tx1.clone()).unwrap();
// 	assert_eq!(hg.conflicts(go.clone(), tx1.inputs.clone()), (vec![tx1.clone()], tx1.clone()));

// 	// A transaction that spends the same inputs.
// 	let tx2 = Tx::new(vec![0, 1], vec![2]);
// 	hg.insert_tx(go.clone(), tx2.clone()).unwrap();
// 	assert_eq!(hg.conflicts(go.clone(), tx2.inputs.clone()), (vec![tx1.clone(), tx2.clone()], tx1.clone()));

// 	// A transaction that spends a distinct inputs.
// 	let tx3 = Tx::new(vec![2, 3], vec![3]);
// 	hg.insert_tx(go.clone(), tx3.clone()).unwrap();
// 	assert_eq!(hg.conflicts(go.clone(), tx3.inputs.clone()), (vec![tx3.clone()], tx3.clone()));

// 	// A transaction that spends multiple conflicting inputs
// 	let tx4 = Tx::new(vec![0, 1, 2, 3], vec![4]);
// 	hg.insert_tx(go.clone(), tx4.clone()).unwrap();
// 	assert_eq!(hg.conflicts(go.clone(), tx4.inputs.clone()), (vec![tx1.clone(), tx2.clone(), tx4.clone(), tx3.clone()], tx1.clone()));
//     }

//     #[actix_rt::test]
//     async fn test_disjoint_inputs() {
// 	// The genesis spendable outputs `go`
// 	let go = Outputs::new(vec![0]);
// 	let mut hg: Hypergraph<u8, u8> = Hypergraph::new(go.clone());

// 	// A transaction that spends `go` and produces a new output
// 	let tx1 = Tx::new(vec![0, 1], vec![1]);
// 	hg.insert_tx(go.clone(), tx1.clone());
// 	assert_eq!(hg.conflicts(go.clone(), tx1.inputs.clone()), (vec![tx1.clone()], tx1.clone()));

// 	// A transaction that spends some of the same inputs as `tx1`
// 	let tx2 = Tx::new(vec![1, 2], vec![2]);
// 	hg.insert_tx(go.clone(), tx2.clone());
// 	assert_eq!(hg.conflicts(go.clone(), tx2.inputs.clone()), (vec![tx1.clone(), tx2.clone()], tx1.clone()));

// 	// A transaction that spends some of te same inputs as `tx2`
// 	let tx3 = Tx::new(vec![2, 3, 4], vec![3]);
// 	hg.insert_tx(go.clone(), tx3.clone());
// 	assert_eq!(hg.conflicts(go.clone(), tx3.inputs.clone()), (vec![tx2.clone(), tx3.clone()], tx2.clone()));

// 	// A transaction that spends one of the same inputs as `tx3`
// 	let tx4 = Tx::new(vec![3], vec![4]);
// 	hg.insert_tx(go.clone(), tx4.clone());
// 	assert_eq!(hg.conflicts(go.clone(), tx4.inputs.clone()), (vec![tx3.clone(), tx4.clone()], tx3.clone()));

// 	// Another transaction that spends one of the same inputs as `tx3`
// 	let tx5 = Tx::new(vec![4], vec![5]);
// 	hg.insert_tx(go.clone(), tx5.clone());
// 	assert_eq!(hg.conflicts(go.clone(), tx5.inputs.clone()), (vec![tx3.clone(), tx5.clone()], tx3.clone()));
//     }

//     #[actix_rt::test]
//     async fn test_outputs() {
// 	// The genesis spendable outputs `go`
// 	let go = Outputs::new(vec![0]);
// 	let mut hg: Hypergraph<u8, u8> = Hypergraph::new(go.clone());

// 	// A transaction that spends `go` and produces two new outputs
// 	let tx1 = Tx::new(vec![0], vec![1, 2]);
// 	hg.insert_tx(go.clone(), tx1.clone()).unwrap();
// 	assert_eq!(hg.conflicts(go.clone(), tx1.inputs.clone()), (vec![tx1.clone()], tx1.clone()));

// 	// A transaction that spends the same inputs as `tx1` and produces the same outputs (duplicate)
// 	let tx2 = Tx::new(vec![0], vec![1, 2]);
// 	hg.insert_tx(go.clone(), tx2.clone()).unwrap();
// 	assert_eq!(hg.conflicts(go.clone(), tx2.inputs.clone()), (vec![tx1.clone()], tx1.clone()));

// 	// A transaction which spends the tx1 outputs and produces new outputs
// 	let tx3 = Tx::new(vec![1, 2], vec![3, 4]);
// 	hg.insert_tx(tx1.outputs.clone(), tx3.clone()).unwrap();
// 	assert_eq!(hg.conflicts(tx1.outputs.clone(), tx3.inputs.clone()), (vec![tx3.clone()], tx3.clone()));

// 	// A transaction which spends tx3 outputs and produces new outputs
// 	let tx4 = Tx::new(vec![3, 4], vec![4, 5]);
// 	hg.insert_tx(tx3.outputs.clone(), tx4.clone()).unwrap();
// 	assert_eq!(hg.conflicts(tx3.outputs.clone(), tx4.inputs.clone()), (vec![tx4.clone()], tx4.clone()));

// 	// A transaction which spends tx3 outputs and conflicts with tx4
// 	let tx5 = Tx::new(vec![3, 4], vec![6, 7]);
// 	hg.insert_tx(tx3.outputs.clone(), tx5.clone()).unwrap();
// 	assert_eq!(hg.conflicts(tx3.outputs.clone(), tx5.inputs.clone()), (vec![tx4.clone(), tx5.clone()], tx4.clone()));

// 	// A transaction which spends tx4 outputs and conflicts in a disjoint manner
// 	let tx6 = Tx::new(vec![3], vec![7]);
// 	hg.insert_tx(tx4.outputs.clone(), tx6.clone()).unwrap();
// 	assert_eq!(hg.conflicts(tx4.outputs.clone(), tx6.inputs.clone()), (vec![tx6.clone()], tx6.clone()));

// 	// A transaction which spends tx4 outputs and conflicts in a disjoint manner
// 	let tx7 = Tx::new(vec![4], vec![8]);
// 	hg.insert_tx(tx4.outputs.clone(), tx7.clone()).unwrap();
// 	assert_eq!(hg.conflicts(tx4.outputs.clone(), tx7.inputs.clone()), (vec![tx7.clone()], tx7.clone()));
//     }

// }

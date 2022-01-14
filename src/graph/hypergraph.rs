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

#[cfg(test)]
mod test {
    use super::Hypergraph;

    use crate::chain::alpha::tx::{Input, Inputs, Output, Outputs, Tx};

    use std::collections::HashSet;

    use ed25519_dalek::Keypair;
    use rand::{rngs::OsRng, CryptoRng};

    fn hash_public(keypair: &Keypair) -> [u8; 32] {
        let enc = bincode::serialize(&keypair.public).unwrap();
        blake3::hash(&enc).as_bytes().clone()
    }

    fn generate_keys() -> (Keypair, Keypair, [u8; 32], [u8; 32]) {
        let mut csprng = OsRng {};
        let kp1 = Keypair::generate(&mut csprng);
        let kp2 = Keypair::generate(&mut csprng);

        let pkh1 = hash_public(&kp1);
        let pkh2 = hash_public(&kp2);

        return (kp1, kp2, pkh1, pkh2);
    }

    #[actix_rt::test]
    async fn test_hypergraph() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        let dummy_tx_hash0 = [0u8; 32];
        let dummy_tx_hash1 = [1u8; 32];

        // Some root unspent outputs for `genesis`.
        let output0 = Output::new(pkh1, 1000);
        let genesis = Outputs::new(vec![output0]);
        let mut hg: Hypergraph = Hypergraph::new(genesis.clone());

        // A transaction that spends `genesis` and produces a new output for `pkh2`.
        let input1 = Input::new(&kp1, dummy_tx_hash0.clone(), 0);
        let output1 = Output::new(pkh2, 1000);
        let tx1 = Tx::new(vec![input1.clone()], vec![output1]);
        hg.insert_tx(genesis.clone(), tx1.clone()).unwrap();
        let c1 = hg.conflicts(genesis.clone(), tx1.inputs.clone());
        assert_eq!(c1, (vec![tx1.clone()], tx1.clone()));

        // A transaction that spends the same input but produces a distinct input should conflict.
        let output2 = Output::new(pkh2, 900);
        let tx2 = Tx::new(vec![input1], vec![output2.clone()]);
        hg.insert_tx(genesis.clone(), tx2.clone()).unwrap();
        let c2 = hg.conflicts(genesis.clone(), tx2.inputs.clone());
        assert_eq!(c2, (vec![tx2.clone(), tx1.clone()], tx1.clone()));

        // A transaction that spends a distinct input.
        let input2 = Input::new(&kp2, dummy_tx_hash1.clone(), 0);
        let tx3 = Tx::new(vec![input2], vec![output2]);
        hg.insert_tx(genesis.clone(), tx3.clone()).unwrap();
        let c3 = hg.conflicts(genesis.clone(), tx3.inputs.clone());
        assert_eq!(c3, (vec![tx3.clone()], tx3.clone()));
    }

    // #[actix_rt::test]
    // async fn test_multiple_inputs() {
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
    // }

    // #[actix_rt::test]
    // async fn test_disjoint_inputs() {
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
    // }

    // #[actix_rt::test]
    // async fn test_outputs() {
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
    // }
}

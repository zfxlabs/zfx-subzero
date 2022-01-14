use super::hyperarc::Hyperarc;
use super::{Error, Result};

use crate::chain::alpha::tx::{Input, Inputs, Output, Outputs, Tx, UTXOIds};

use crate::sleet::conflict_set::ConflictSet;

use std::collections::{hash_map::Entry, HashMap, HashSet};

#[derive(Debug)]
pub struct Hypergraph {
    /// The adjacency lists of `H`. Each edge `I` can point to more than one vertex.
    h: HashMap<UTXOIds, Hyperarc>,
}

impl std::ops::Deref for Hypergraph {
    type Target = HashMap<UTXOIds, Hyperarc>;

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
    pub fn new(g: UTXOIds) -> Self {
        let mut map: HashMap<UTXOIds, Hyperarc> = HashMap::default();
        let _ = map.insert(g.clone(), Hyperarc::new());
        Hypergraph { h: map }
    }

    pub fn insert_tx(&mut self, tx: Tx) -> Result<()> {
        // Fetch the spent output UTXO ids from the inputs.
        let spent_outputs = UTXOIds::new(tx.inputs.clone());
        // Check if there is an entry for the supplied `UTXOIds`.
        match self.entry(spent_outputs.clone()) {
            Entry::Occupied(mut o) => {
                let hyperarc = o.get_mut();
                match hyperarc.get(&tx.inputs) {
                    // If there is already an equivalent inputs edge for the outputs being spent, then
                    // there is a conflict. Thus we should update the hyperarc.
                    Some(existing) => {
                        let () = hyperarc.update(tx);
                    }
                    // Otherwise we create a new edge for this tx.
                    None => {
                        let _ = hyperarc.insert_new(tx).unwrap();
                    }
                }
            }
            // If the outputs being spent do not exist then we are attempting to spend from a new
            // set of UTXOIds. In this case we create a new hyperarc and conflicting set.
            Entry::Vacant(mut v) => {
                let mut hyperarc = Hyperarc::new();
                let _ = hyperarc.insert_new(tx).unwrap();
                let _ = v.insert(hyperarc);
            }
        }
        Ok(())
    }

    pub fn conflicts(&self, inputs: Inputs<Input>) -> ConflictSet<Tx> {
        let spent_outputs = UTXOIds::new(inputs.clone());
        let hyperarc = self.get(&spent_outputs).unwrap();
        hyperarc.get(&inputs).unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::Hypergraph;

    use crate::chain::alpha::tx::{Input, Inputs, Output, Outputs, Tx, UTXOIds};
    use crate::sleet::conflict_set::ConflictSet;

    use std::collections::HashSet;

    use ed25519_dalek::Keypair;

    #[actix_rt::test]
    async fn test_hypergraph() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        let dummy_tx_hash0 = [0u8; 32];
        let dummy_tx_hash1 = [1u8; 32];
        let dummy_tx_hash2 = [2u8; 32];

        // Some root unspent outputs for `genesis`. We assume this input refers to a UTXO with funds
        // but for the purposes of the hypergraph it doesn't matter.
        let input1 = Input::new(&kp1, dummy_tx_hash0.clone(), 0);
        let genesis = UTXOIds::new(Inputs::new(vec![input1]));
        let mut hg: Hypergraph = Hypergraph::new(genesis.clone());

        // A transaction that spends `genesis` and produces a new output for `pkh2`.
        let input2 = Input::new(&kp1, dummy_tx_hash1.clone(), 0);
        let output1 = Output::new(pkh2, 1000);
        let tx1 = Tx::new(vec![input2.clone()], vec![output1.clone()]);
        hg.insert_tx(tx1.clone()).unwrap();
        assert_eq!(hg.conflicts(tx1.inputs.clone()), conflict_set(vec![tx1.clone()], tx1.clone()));

        // A transaction that spends the same input but produces a distinct output should conflict.
        let output2 = Output::new(pkh2, 900);
        let tx2 = Tx::new(vec![input2.clone()], vec![output2.clone()]);
        hg.insert_tx(tx2.clone()).unwrap();
        assert_eq!(
            hg.conflicts(tx2.inputs.clone()),
            conflict_set(vec![tx2.clone(), tx1.clone()], tx1.clone())
        );

        // A transaction that spends a distinct input should not conflict.
        let input3 = Input::new(&kp2, dummy_tx_hash2.clone(), 0);
        let tx3 = Tx::new(vec![input3], vec![output2]);
        hg.insert_tx(tx3.clone()).unwrap();
        assert_eq!(hg.conflicts(tx3.inputs.clone()), conflict_set(vec![tx3.clone()], tx3.clone()));
    }

    // #[actix_rt::test]
    // async fn test_multiple_inputs() {
    //     let (kp1, kp2, pkh1, pkh2) = generate_keys();

    //     let dummy_tx_hash0 = [0u8; 32];
    //     let dummy_tx_hash1 = [1u8; 32];

    //     // Some root unspent outputs for `genesis`.
    //     let output0 = Output::new(pkh1, 1000);
    //     let genesis = Outputs::new(vec![output0]);
    //     let mut hg: Hypergraph = Hypergraph::new(genesis.clone());

    // 	// A transaction that spends `genesis` and produces a new output.
    //     let input1 = Input::new(&kp1, dummy_tx_hash0.clone(), 0);
    //     let output1 = Output::new(pkh2, 1000);
    // 	let tx1 = Tx::new(vec![input1.clone()], vec![output1]);
    // 	hg.insert_tx(genesis.clone(), tx1.clone()).unwrap();
    // 	let c1 = hg.conflicts(genesis.clone(), tx1.inputs.clone());
    // 	assert_eq!(c1, (vec![tx1.clone()], tx1.clone()));

    // 	// A transaction that spends the same inputs.
    //     let output2 = Output::new(pkh2, 900);
    // 	let tx2 = Tx::new(vec![input1.clone()], vec![output2.clone()]);
    // 	hg.insert_tx(genesis.clone(), tx2.clone()).unwrap();
    // 	let c2 = hg.conflicts(genesis.clone(), tx2.inputs.clone());
    // 	assert_eq!(c2, (vec![tx2.clone(), tx1.clone()], tx1.clone()));

    // 	// A transaction that spends a distinct inputs but produces the same outputs.
    //     let input2 = Input::new(&kp2, dummy_tx_hash1.clone(), 0);
    //     let tx3 = Tx::new(vec![input2.clone()], vec![output2]);
    //     hg.insert_tx(genesis.clone(), tx3.clone()).unwrap();
    //     let c3 = hg.conflicts(genesis.clone(), tx3.inputs.clone());
    //     assert_eq!(c3, (vec![tx3.clone()], tx3.clone()));

    // 	// A transaction that spends multiple conflicting inputs
    // 	let output3 = Output::new(pkh2, 800);
    // 	let tx4 = Tx::new(vec![input1.clone(), input2.clone()], vec![output3]);
    // 	hg.insert_tx(genesis.clone(), tx4.clone()).unwrap();
    // 	let c4 = hg.conflicts(genesis.clone(), tx4.inputs.clone());
    // 	assert_eq!(c4, (vec![tx2.clone(), tx1.clone(), tx4.clone(), tx3.clone()], tx1.clone()));
    // }

    // #[actix_rt::test]
    // async fn test_disjoint_inputs() {
    //     let (kp1, kp2, pkh1, pkh2) = generate_keys();

    //     let dummy_tx_hash0 = [0u8; 32];
    //     let dummy_tx_hash1 = [1u8; 32];
    //     let dummy_tx_hash2 = [2u8; 32];
    //     let dummy_tx_hash3 = [3u8; 32];

    //     // Some root unspent outputs for `genesis`.
    //     let output0 = Output::new(pkh1, 1000);
    //     let genesis = Outputs::new(vec![output0]);
    //     let mut hg: Hypergraph = Hypergraph::new(genesis.clone());

    // 	// A transaction that spends `genesis` and produces a new output
    //     let input1 = Input::new(&kp1, dummy_tx_hash0.clone(), 0);
    //     let output1 = Output::new(pkh2, 1000);
    // 	let tx1 = Tx::new(vec![input1.clone()], vec![output1]);
    // 	hg.insert_tx(genesis.clone(), tx1.clone()).unwrap();
    // 	let c1 = hg.conflicts(genesis.clone(), tx1.inputs.clone());
    // 	assert_eq!(c1, (vec![tx1.clone()], tx1.clone()));

    // 	// A transaction that spends some of the same inputs as `tx1`
    //     let output2 = Output::new(pkh2, 900);
    //     let input2 = Input::new(&kp2, dummy_tx_hash1.clone(), 0);
    // 	let tx2 = Tx::new(vec![input1.clone(), input2.clone()], vec![output2.clone()]);
    // 	hg.insert_tx(genesis.clone(), tx2.clone()).unwrap();
    // 	let c2 = hg.conflicts(genesis.clone(), tx2.inputs.clone());
    // 	assert_eq!(c2, (vec![tx1.clone(), tx2.clone()], tx1.clone()));

    // 	// A transaction that spends some of te same inputs as `tx2`
    // 	let output3 = Output::new(pkh2, 800);
    //     let input3 = Input::new(&kp2, dummy_tx_hash2.clone(), 0);
    //     let input4 = Input::new(&kp2, dummy_tx_hash3.clone(), 0);
    // 	let tx3 = Tx::new(vec![input2.clone(), input3.clone(), input4.clone()], vec![output3.clone()]);
    // 	hg.insert_tx(genesis.clone(), tx3.clone()).unwrap();
    // 	let c3 = hg.conflicts(genesis.clone(), tx3.inputs.clone());
    // 	assert_eq!(c3, (vec![tx2.clone(), tx3.clone()], tx2.clone()));

    // 	// A transaction that spends one of the same inputs as `tx3`
    // 	let output4 = Output::new(pkh2, 700);
    // 	let tx4 = Tx::new(vec![input3.clone()], vec![output4.clone()]);
    // 	hg.insert_tx(genesis.clone(), tx4.clone());
    // 	let c4 = hg.conflicts(genesis.clone(), tx4.inputs.clone());
    // 	assert_eq!(c4, (vec![tx3.clone(), tx4.clone()], tx3.clone()));

    // 	// Another transaction that spends one of the same inputs as `tx3`
    // 	let output5 = Output::new(pkh2, 600);
    // 	let tx5 = Tx::new(vec![input4.clone()], vec![output5.clone()]);
    // 	hg.insert_tx(genesis.clone(), tx5.clone());
    // 	let c5 = hg.conflicts(genesis.clone(), tx5.inputs.clone());
    // 	assert_eq!(c5, (vec![tx3.clone(), tx5.clone()], tx3.clone()));
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

    fn conflict_set(conflicts: Vec<Tx>, pref: Tx) -> ConflictSet<Tx> {
        ConflictSet {
            conflicts: conflicts.iter().cloned().collect(),
            pref: pref.clone(),
            last: pref,
            cnt: 0u8,
        }
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

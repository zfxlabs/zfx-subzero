use super::{Error, Result};

use crate::chain::alpha::tx::{Input, Inputs, Tx, TxHash, UTXOIds};

use crate::sleet::conflict_set::ConflictSet;

use std::collections::{hash_map::Entry, HashMap, HashSet};

pub struct Hypergraph {
    dh: HashMap<UTXOIds, Vec<(Inputs<Input>, Tx, UTXOIds)>>,
    // Maintains an ordered set of transactions for conflict set preference.
    cs: Vec<(TxHash, ConflictSet<Tx>)>,
}

impl Hypergraph {
    pub fn new(genesis: UTXOIds) -> Self {
        let mut adj = HashMap::default();
        adj.insert(genesis, vec![]);
        Hypergraph {
            dh: adj,
            // Note: genesis cannot conflict.
            cs: vec![],
        }
    }

    pub fn insert_tx(&mut self, tx: Tx) -> Result<()> {
        // The utxo ids that this transaction consumes.
        let mut consumed_utxo_ids = UTXOIds::from_inputs(tx.inputs.clone());
        // If there exists an intersecting set of utxo ids in the hypergraph then we are
        // spending from those outputs.
        let mut intersecting_vertices = HashSet::new();
        for (utxo_ids, _) in self.dh.iter() {
            if consumed_utxo_ids.intersects_with(utxo_ids) {
                intersecting_vertices.insert(utxo_ids.clone());
                // Remove the intersecting utxo ids.
                consumed_utxo_ids = consumed_utxo_ids.left_difference(utxo_ids);
            }
        }
        // If we did not succeed in finding all the utxo ids being consumed then this is
        // an error - an entry must exist in order to be spent.
        if consumed_utxo_ids.len() > 0 {
            return Err(Error::UndefinedUTXO);
        } else {
            let produced_utxo_ids = UTXOIds::from_outputs(tx.hash(), tx.outputs.clone());
            // First we make sure that the produced `utxo_ids` exist within the hypergraph.
            match self.dh.entry(produced_utxo_ids.clone()) {
                // If the produced utxo ids already exist then we have an error - a duplicate
                // transaction exists in the hypergraph. This implies that the transaction
                // had the same hash as another transaction through the hash in `from_outputs`.
                Entry::Occupied(_) => return Err(Error::DuplicateUTXO),
                // Otherwise we create an empty entry (same as when creating genesis).
                Entry::Vacant(mut v) => {
                    let _ = v.insert(vec![]);
                }
            }

            // Next we make sure that there is a conflict set for this tx.
            for i in 0..self.cs.len() {
                if self.cs[i].0 == tx.hash() {
                    return Err(Error::DuplicateUTXO);
                }
            }
            let tx_i = self.cs.len();
            self.cs.push((tx.hash(), ConflictSet::new(tx.clone())));

            // Next we produce an arc from the subset of inputs relevant to the spent
            // vertex directed to the produced utxo ids.
            let consumed_utxo_ids = UTXOIds::from_inputs(tx.inputs.clone());
            let mut conflicts = vec![];
            for utxo_ids in intersecting_vertices.iter() {
                // ( subset of `tx.inputs` relevant to `vi`, conflicting tx, `produced_utxo_ids` )
                match self.dh.entry(utxo_ids.clone()) {
                    Entry::Occupied(mut o) => {
                        let arcs = o.get_mut();
                        let intersection = utxo_ids.intersect(&consumed_utxo_ids);
                        let mut intersecting_inputs = vec![];
                        for utxo_id in intersection.iter() {
                            for input in tx.inputs.iter() {
                                if input.utxo_id() == utxo_id.clone() {
                                    intersecting_inputs.push(input.clone());
                                }
                            }
                        }

                        // Save existing conflicting transactions.
                        for (inputs, conflicting_tx, _) in arcs.iter() {
                            if !inputs.is_disjoint(&tx.inputs) {
                                conflicts.push(conflicting_tx.clone());
                            }
                        }

                        // FIXME: If the `arc` we are pushing is identical to another arc, then do
                        // not push it into the arcs - error.

                        // Push a new arc.
                        arcs.push((
                            Inputs::new(intersecting_inputs),
                            tx.clone(),
                            produced_utxo_ids.clone(),
                        ));
                    }
                    Entry::Vacant(_) => return Err(Error::UndefinedUTXO),
                }
            }
            // For all the transactions that we conflict with, we wish to add the conflicts to the
            // conflicts sets of this transaction and any conflicting transactions, whilst saving
            // the transactions by order of preference - this is determined by insertion order.
            let mut ordered_conflicting_txs = vec![];
            let mut pref = None;
            let mut last = None;
            let mut cnt = 0u8;
            for conflicting_tx in conflicts.iter() {
                for i in 0..self.cs.len() {
                    if self.cs[i].0 == conflicting_tx.hash() {
                        // Note: We do not change the properties of the conflict set since this one
                        // came first and is thus preferred.
                        self.cs[i].1.conflicts.insert(tx.clone());
                        if pref.is_none() {
                            pref = Some(self.cs[i].1.pref.clone());
                            last = Some(self.cs[i].1.last.clone());
                            cnt = self.cs[i].1.cnt.clone();
                        }
                        ordered_conflicting_txs.push(conflicting_tx.clone());
                    }
                }
            }
            // Update the conflict set of this transaction based on the ordered txs.
            if ordered_conflicting_txs.len() > 0 {
                self.cs[tx_i].1.pref = pref.unwrap();
                // FIXME: Not sure here.
                self.cs[tx_i].1.last = last.unwrap();
                self.cs[tx_i].1.cnt = cnt;
                for ordered_tx in ordered_conflicting_txs.iter().cloned() {
                    println!("inserting = {:?}", ordered_tx.outputs.clone());
                    self.cs[tx_i].1.conflicts.insert(ordered_tx);
                }
            }
        }
        Ok(())
    }

    pub fn conflicting_txs(&mut self, tx: Tx) -> Option<ConflictSet<Tx>> {
        if self.cs.len() > 0 {
            for i in 0..self.cs.len() {
                if self.cs[i].0 == tx.hash() {
                    return Some(self.cs[i].1.clone());
                }
            }
            None
        } else {
            None
        }
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

        // Some root unspent outputs for `genesis`. We assume this input refers to a UTXO with funds
        // but for the purposes of the hypergraph it doesn't matter.
        let genesis_tx = Tx::new(
            vec![],
            vec![Output::new(pkh1, 1000), Output::new(pkh2, 1000), Output::new(pkh2, 500)],
        );

        let genesis = UTXOIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs.clone());
        let mut dh: Hypergraph = Hypergraph::new(genesis.clone());

        let input1 = Input::new(&kp1, genesis_tx.hash(), 0);
        let input2 = Input::new(&kp2, genesis_tx.hash(), 1);
        let input3 = Input::new(&kp2, genesis_tx.hash(), 2);

        // A transaction that spends `genesis` and produces a new output for `pkh2`.
        let output1 = Output::new(pkh2, 1000);
        let tx1 = Tx::new(vec![input1.clone()], vec![output1.clone()]);
        dh.insert_tx(tx1.clone()).unwrap();
        let expected: HashSet<Tx> = vec![tx1.clone()].iter().cloned().collect();
        let c1 = dh.conflicting_txs(tx1.clone()).unwrap();
        assert_eq!(c1.conflicts.len(), 1);
        assert_eq!(c1.conflicts, expected);
        assert_eq!(c1.pref, tx1.clone());

        // A transaction that spends the same inputs but produces a distinct output should conflict.
        let output2 = Output::new(pkh2, 900);
        let tx2 = Tx::new(vec![input1.clone(), input2.clone()], vec![output2.clone()]);
        dh.insert_tx(tx2.clone()).unwrap();
        let expected: HashSet<Tx> = vec![tx1.clone(), tx2.clone()].iter().cloned().collect();
        let c2 = dh.conflicting_txs(tx2.clone()).unwrap();
        assert_eq!(c2.conflicts.len(), 2);
        assert_eq!(c2.conflicts, expected);
        assert_eq!(c2.pref, tx1.clone());

        // A transaction that spends a distinct input should not conflict.
        let tx3 = Tx::new(vec![input3.clone()], vec![output2.clone()]);
        dh.insert_tx(tx3.clone()).unwrap();
        let expected: HashSet<Tx> = vec![tx3.clone()].iter().cloned().collect();
        let c3 = dh.conflicting_txs(tx3.clone()).unwrap();
        assert_eq!(c3.conflicts.len(), 1);
        assert_eq!(c3.conflicts, expected);
        assert_eq!(c3.pref, tx3.clone());
    }

    #[actix_rt::test]
    async fn test_multiple_inputs() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        // Some root unspent outputs for `genesis`. We assume this input refers to a UTXO with funds
        // but for the purposes of the hypergraph it doesn't matter.
        let genesis_tx = Tx::new(
            vec![],
            vec![Output::new(pkh1, 1000), Output::new(pkh2, 1000), Output::new(pkh2, 500)],
        );

        let genesis = UTXOIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs.clone());
        let mut dh: Hypergraph = Hypergraph::new(genesis.clone());

        let input1 = Input::new(&kp1, genesis_tx.hash(), 0);
        let input2 = Input::new(&kp2, genesis_tx.hash(), 1);
        let input3 = Input::new(&kp2, genesis_tx.hash(), 2);

        // A transaction that spends `genesis` and produces a new output for `pkh2`.
        let output1 = Output::new(pkh2, 1000);
        let tx1 = Tx::new(vec![input1.clone()], vec![output1.clone()]);
        dh.insert_tx(tx1.clone()).unwrap();
        let expected: HashSet<Tx> = vec![tx1.clone()].iter().cloned().collect();
        let c1 = dh.conflicting_txs(tx1.clone()).unwrap();
        assert_eq!(c1.conflicts.len(), 1);
        assert_eq!(c1.conflicts, expected);
        assert_eq!(c1.pref, tx1.clone());

        // A transaction that spends the same inputs but produces a distinct output should conflict.
        let output2 = Output::new(pkh2, 900);
        let tx2 = Tx::new(vec![input1.clone(), input2.clone()], vec![output2.clone()]);
        dh.insert_tx(tx2.clone()).unwrap();
        let expected: HashSet<Tx> = vec![tx1.clone(), tx2.clone()].iter().cloned().collect();
        let c2 = dh.conflicting_txs(tx2.clone()).unwrap();
        assert_eq!(c2.conflicts.len(), 2);
        assert_eq!(c2.conflicts, expected);
        assert_eq!(c2.pref, tx1.clone());

        // A transaction that spends a distinct input should not conflict.
        let tx3 = Tx::new(vec![input3.clone()], vec![output2.clone()]);
        dh.insert_tx(tx3.clone()).unwrap();
        let expected: HashSet<Tx> = vec![tx3.clone()].iter().cloned().collect();
        let c3 = dh.conflicting_txs(tx3.clone()).unwrap();
        assert_eq!(c3.conflicts.len(), 1);
        assert_eq!(c3.conflicts, expected);
        assert_eq!(c3.pref, tx3.clone());

        // A transaction that spends multiple conflicting inputs
        let output3 = Output::new(pkh2, 800);
        let tx4 = Tx::new(vec![input1.clone(), input2.clone(), input3.clone()], vec![output3]);
        dh.insert_tx(tx4.clone()).unwrap();
        let expected: HashSet<Tx> =
            vec![tx1.clone(), tx2.clone(), tx3.clone(), tx4.clone()].iter().cloned().collect();
        let c4 = dh.conflicting_txs(tx4.clone()).unwrap();
        assert_eq!(c4.conflicts.len(), 4);
        assert_eq!(c4.conflicts, expected);
        assert_eq!(c4.pref, tx1.clone());
    }

    #[actix_rt::test]
    async fn test_disjoint_inputs() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        // Some root unspent outputs for `genesis`. We assume this input refers to a UTXO with funds
        // but for the purposes of the hypergraph it doesn't matter.
        let genesis_tx = Tx::new(
            vec![],
            vec![
                Output::new(pkh1, 1000),
                Output::new(pkh2, 1000),
                Output::new(pkh2, 500),
                Output::new(pkh2, 400),
            ],
        );

        let genesis = UTXOIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs.clone());
        let mut dh: Hypergraph = Hypergraph::new(genesis.clone());

        let input1 = Input::new(&kp1, genesis_tx.hash(), 0);
        let input2 = Input::new(&kp2, genesis_tx.hash(), 1);
        let input3 = Input::new(&kp2, genesis_tx.hash(), 2);
        let input4 = Input::new(&kp2, genesis_tx.hash(), 3);

        // A transaction that spends `genesis` and produces a new output for `pkh2`.
        let output1 = Output::new(pkh2, 1000);
        let tx1 = Tx::new(vec![input1.clone()], vec![output1.clone()]);
        dh.insert_tx(tx1.clone()).unwrap();
        let expected: HashSet<Tx> = vec![tx1.clone()].iter().cloned().collect();
        let c1 = dh.conflicting_txs(tx1.clone()).unwrap();
        assert_eq!(c1.conflicts.len(), 1);
        assert_eq!(c1.conflicts, expected);
        assert_eq!(c1.pref, tx1.clone());

        // A transaction that spends some of the same inputs as `tx1`
        let output2 = Output::new(pkh2, 900);
        let tx2 = Tx::new(vec![input1.clone(), input2.clone()], vec![output2.clone()]);
        dh.insert_tx(tx2.clone()).unwrap();
        let expected: HashSet<Tx> = vec![tx1.clone(), tx2.clone()].iter().cloned().collect();
        let c2 = dh.conflicting_txs(tx2.clone()).unwrap();
        assert_eq!(c2.conflicts.len(), 2);
        assert_eq!(c2.conflicts, expected);
        assert_eq!(c2.pref, tx1.clone());

        // A transaction that spends some of the same inputs as `tx2`
        let output3 = Output::new(pkh2, 800);
        let tx3 =
            Tx::new(vec![input2.clone(), input3.clone(), input4.clone()], vec![output3.clone()]);
        dh.insert_tx(tx3.clone()).unwrap();
        let expected: HashSet<Tx> = vec![tx2.clone(), tx3.clone()].iter().cloned().collect();
        let c3 = dh.conflicting_txs(tx3.clone()).unwrap();
        assert_eq!(c3.conflicts.len(), 2);
        assert_eq!(c3.conflicts, expected);
        assert_eq!(c3.pref, tx1.clone());

        // A transaction that spends one of the same inputs as `tx3`
        let output4 = Output::new(pkh2, 700);
        let tx4 = Tx::new(vec![input3.clone()], vec![output4.clone()]);
        dh.insert_tx(tx4.clone()).unwrap();
        let expected: HashSet<Tx> = vec![tx3.clone(), tx4.clone()].iter().cloned().collect();
        let c4 = dh.conflicting_txs(tx4.clone()).unwrap();
        assert_eq!(c4.conflicts.len(), 2);
        assert_eq!(c4.conflicts, expected);
        assert_eq!(c4.pref, tx1.clone());

        // Another transaction that spends one of the same inputs as `tx3`
        let output5 = Output::new(pkh2, 600);
        let tx5 = Tx::new(vec![input4.clone()], vec![output5.clone()]);
        dh.insert_tx(tx5.clone()).unwrap();
        let expected: HashSet<Tx> = vec![tx3.clone(), tx5.clone()].iter().cloned().collect();
        let c5 = dh.conflicting_txs(tx5.clone()).unwrap();
        assert_eq!(c5.conflicts.len(), 2);
        assert_eq!(c5.conflicts, expected);
        assert_eq!(c5.pref, tx1.clone());
    }

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

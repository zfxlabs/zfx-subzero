use super::{Error, Result};

use crate::chain::alpha::tx::{Input, Inputs, Transaction, TxHash, UTXOIds};

use crate::sleet::conflict_set::ConflictSet;

use std::collections::{hash_map::Entry, HashMap, HashSet};

pub struct UTXOGraph {
    dh: HashMap<UTXOIds, Vec<Transaction>>,
    // Maintains an ordered set of transactions for conflict set preference.
    cs: Vec<(TxHash, ConflictSet<TxHash>)>,
}

impl UTXOGraph {
    pub fn new(genesis: UTXOIds) -> Self {
        let mut adj = HashMap::default();
        adj.insert(genesis, vec![]);
        UTXOGraph {
            dh: adj,
            // Note: genesis cannot conflict.
            cs: vec![],
        }
    }

    pub fn insert_tx(&mut self, tx: Transaction) -> Result<()> {
        // The utxo ids that this transaction consumes.
        let mut consumed_utxo_ids = UTXOIds::from_inputs(tx.inputs());
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
            let produced_utxo_ids = UTXOIds::from_outputs(tx.hash(), tx.outputs());
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
            self.cs.push((tx.hash(), ConflictSet::new(tx.hash())));

            // For each set of intersecting vertices (UTXOId bundles) an arc is produced relating the
            // UTXOIds to the new transaction.
            let consumed_utxo_ids = UTXOIds::from_inputs(tx.inputs());
            let mut conflicts = vec![];
            for utxo_ids in intersecting_vertices.iter() {
                match self.dh.entry(utxo_ids.clone()) {
                    Entry::Occupied(mut o) => {
                        let arcs = o.get_mut();

                        // Save existing conflicting transactions.
                        for arc_tx in arcs.iter() {
                            if arc_tx.clone() == tx.clone() {
                                return Err(Error::DuplicateUTXO);
                            }
                            if !arc_tx.inputs().is_disjoint(&tx.inputs()) {
                                conflicts.push(arc_tx.clone());
                            }
                        }
                        arcs.push(tx.clone());
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
                        self.cs[i].1.conflicts.insert(tx.hash());
                        // Save the properties of the first conflict set (the most preferred).
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
                    self.cs[tx_i].1.conflicts.insert(ordered_tx.hash());
                }
            }
        }
        Ok(())
    }

    pub fn accept_tx(&mut self, tx: Transaction) -> Result<()> {
        // Once a transaction is accepted we wish to remove all the conflicts from the graph
        // in order to free up space for future entries.
        match self.conflicting_txs(&tx.hash()) {
            Some(conflict_set) => {
                // If the transaction does not conflict then we are done.
                if conflict_set.is_singleton() {
                    return Ok(());
                }

                // First fetch all the conflicting utxo ids produced by the conflicting txs,
                // excluding the `tx` being accepted.
                let mut conflicting_utxo_ids = vec![];
                for conflicting_tx_hash in conflict_set.conflicts.iter() {
                    if tx.hash().eq(conflicting_tx_hash) {
                        continue;
                    }
                    let utxo_ids = UTXOIds::from_outputs(tx.hash(), tx.outputs());
                    conflicting_utxo_ids.push(utxo_ids);
                }

                // Next remove each vertex from the graph which is a conflicting `utxo_id`.
                for conflicting_utxo_id in conflicting_utxo_ids.iter() {
                    self.dh.remove(&conflicting_utxo_id).unwrap();
                }

                // Next remove each arc which point to the conflicting transactions (which no
                // longer exist).
                for (_, arcs) in self.dh.iter_mut() {
                    for i in 0..arcs.len() {
                        if arcs[i].clone() == tx.clone() {
                            continue;
                        }
                        if conflict_set.conflicts.contains(&arcs[i].hash()) {
                            arcs.remove(i);
                        }
                    }
                }

                // Next remove the conflicting transactions from the conflict sets, preserving
                // the ordering.
                let mut cs: Vec<(TxHash, ConflictSet<TxHash>)> = vec![];
                for i in 0..self.cs.len() {
                    if self.cs[i].0 == tx.hash() {
                        cs.push((tx.hash(), ConflictSet::new(tx.hash())));
                    }
                    let mut conflicts = false;
                    for conflicting_tx_hash in conflict_set.conflicts.iter() {
                        if self.cs[i].0.eq(conflicting_tx_hash) {
                            conflicts = true;
                            break;
                        }
                    }
                    if conflicts {
                        continue;
                    } else {
                        // TODO: Remove the conflicting transactions the existing preserved
                        // conflict sets.
                        cs.push((self.cs[i].0, self.cs[i].1.clone()));
                    }
                }
                self.cs = cs;

                Ok(())
            }
            // If the transaction has no conflict set then it is invalid.
            None => Err(Error::UndefinedUTXO),
        }
    }

    pub fn conflicting_txs(&self, tx_hash: &TxHash) -> Option<ConflictSet<TxHash>> {
        if self.cs.len() > 0 {
            for i in 0..self.cs.len() {
                if self.cs[i].0.eq(tx_hash) {
                    return Some(self.cs[i].1.clone());
                }
            }
            None
        } else {
            None
        }
    }

    pub fn is_singleton(&self, tx_hash: &TxHash) -> Result<bool> {
        match self.conflicting_txs(tx_hash) {
            Some(conflict_set) => Ok(conflict_set.is_singleton()),
            None => Err(Error::InvalidTxHash(tx_hash.clone())),
        }
    }

    pub fn get_preferred(&self, tx_hash: &TxHash) -> Result<TxHash> {
        match self.conflicting_txs(tx_hash) {
            Some(conflict_set) => Ok(conflict_set.pref),
            None => Err(Error::InvalidTxHash(tx_hash.clone())),
        }
    }

    pub fn is_preferred(&self, tx_hash: &TxHash) -> Result<bool> {
        match self.conflicting_txs(tx_hash) {
            Some(conflict_set) => Ok(conflict_set.is_preferred(tx_hash.clone())),
            None => Err(Error::InvalidTxHash(tx_hash.clone())),
        }
    }

    pub fn get_confidence(&self, tx_hash: &TxHash) -> Result<u8> {
        match self.conflicting_txs(tx_hash) {
            Some(conflict_set) => Ok(conflict_set.cnt),
            None => Err(Error::InvalidTxHash(tx_hash.clone())),
        }
    }

    pub fn update_conflict_set(&mut self, tx_hash: &TxHash, d1: u8, d2: u8) -> Result<()> {
        if self.cs.len() > 0 {
            for i in 0..self.cs.len() {
                if self.cs[i].0.eq(tx_hash) {
                    if d1 > d2 {
                        self.cs[i].1.pref = tx_hash.clone();
                    }
                    if !tx_hash.eq(&self.cs[i].1.last) {
                        self.cs[i].1.last = tx_hash.clone();
                    } else {
                        self.cs[i].1.cnt += 1;
                    }
                    return Ok(());
                }
            }
            Err(Error::InvalidTxHash(tx_hash.clone()))
        } else {
            Err(Error::EmptyUTXOGraph)
        }
    }
}

#[cfg(test)]
mod test {
    use super::UTXOGraph;

    use crate::chain::alpha::tx::{CoinbaseTx, TransferTx};
    use crate::chain::alpha::tx::{
        Input, Inputs, Output, Outputs, Transaction, Tx, TxHash, UTXOIds,
    };
    use crate::sleet::conflict_set::ConflictSet;

    use std::collections::HashSet;

    use ed25519_dalek::Keypair;

    #[actix_rt::test]
    async fn test_utxo_graph() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        // Some root unspent outputs for `genesis`. We assume this input refers to a UTXO with funds
        // but for the purposes of the hypergraph it doesn't matter.
        let genesis_coinbase_tx = CoinbaseTx::new(Outputs::new(vec![
            Output::new(pkh1, 1000),
            Output::new(pkh2, 1000),
            Output::new(pkh2, 500),
        ]));
        let genesis_tx = Transaction::CoinbaseTx(genesis_coinbase_tx);
        let genesis = UTXOIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs());
        let mut dh: UTXOGraph = UTXOGraph::new(genesis.clone());

        let input1 = Input::new(&kp1, genesis_tx.hash(), 0);
        let input2 = Input::new(&kp2, genesis_tx.hash(), 1);
        let input3 = Input::new(&kp2, genesis_tx.hash(), 2);

        // A transaction that spends `genesis` and produces a new output for `pkh2`.
        let transfer_tx =
            TransferTx::new(&kp2, genesis_tx.clone(), pkh2.clone(), pkh1.clone(), 500);
        let tx1 = Transaction::TransferTx(transfer_tx);
        dh.insert_tx(tx1.clone()).unwrap();
        let expected: HashSet<TxHash> = vec![tx1.hash()].iter().cloned().collect();
        let c1 = dh.conflicting_txs(&tx1.hash()).unwrap();
        assert_eq!(c1.conflicts.len(), 1);
        assert_eq!(c1.conflicts, expected);
        assert_eq!(c1.pref, tx1.hash());

        // A transaction that spends the same inputs but produces a distinct output should conflict.
        let transfer_tx =
            TransferTx::new(&kp2, genesis_tx.clone(), pkh2.clone(), pkh1.clone(), 550);
        let tx2 = Transaction::TransferTx(transfer_tx);
        dh.insert_tx(tx2.clone()).unwrap();
        let expected: HashSet<TxHash> = vec![tx1.hash(), tx2.hash()].iter().cloned().collect();
        let c2 = dh.conflicting_txs(&tx2.hash()).unwrap();
        assert_eq!(c2.conflicts.len(), 2);
        assert_eq!(c2.conflicts, expected);
        assert_eq!(c2.pref, tx1.hash());

        // A transaction that spends a distinct input should not conflict.
        let transfer_tx =
            TransferTx::new(&kp1, genesis_tx.clone(), pkh2.clone(), pkh1.clone(), 550);
        let tx3 = Transaction::TransferTx(transfer_tx);
        dh.insert_tx(tx3.clone()).unwrap();
        let expected: HashSet<TxHash> = vec![tx3.hash()].iter().cloned().collect();
        let c3 = dh.conflicting_txs(&tx3.hash()).unwrap();
        assert_eq!(c3.conflicts.len(), 1);
        assert_eq!(c3.conflicts, expected);
        assert_eq!(c3.pref, tx3.hash());
    }

    // #[actix_rt::test]
    // async fn test_multiple_inputs() {
    //     let (kp1, kp2, pkh1, pkh2) = generate_keys();

    //     // Some root unspent outputs for `genesis`. We assume this input refers to a UTXO with funds
    //     // but for the purposes of the hypergraph it doesn't matter.
    //     let genesis_tx = Tx::new(
    //         vec![],
    //         vec![Output::new(pkh1, 1000), Output::new(pkh2, 1000), Output::new(pkh2, 500)],
    //     );

    //     let genesis = UTXOIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs.clone());
    //     let mut dh: UTXOGraph = UTXOGraph::new(genesis.clone());

    //     let input1 = Input::new(&kp1, genesis_tx.hash(), 0);
    //     let input2 = Input::new(&kp2, genesis_tx.hash(), 1);
    //     let input3 = Input::new(&kp2, genesis_tx.hash(), 2);

    //     // A transaction that spends `genesis` and produces a new output for `pkh2`.
    //     let output1 = Output::new(pkh2, 1000);
    //     let tx1 = Tx::new(vec![input1.clone()], vec![output1.clone()]);
    //     dh.insert_tx(tx1.clone()).unwrap();
    //     let expected: HashSet<Tx> = vec![tx1.clone()].iter().cloned().collect();
    //     let c1 = dh.conflicting_txs(tx1.clone()).unwrap();
    //     assert_eq!(c1.conflicts.len(), 1);
    //     assert_eq!(c1.conflicts, expected);
    //     assert_eq!(c1.pref, tx1.clone());

    //     // A transaction that spends the same inputs but produces a distinct output should conflict.
    //     let output2 = Output::new(pkh2, 900);
    //     let tx2 = Tx::new(vec![input1.clone(), input2.clone()], vec![output2.clone()]);
    //     dh.insert_tx(tx2.clone()).unwrap();
    //     let expected: HashSet<Tx> = vec![tx1.clone(), tx2.clone()].iter().cloned().collect();
    //     let c2 = dh.conflicting_txs(tx2.clone()).unwrap();
    //     assert_eq!(c2.conflicts.len(), 2);
    //     assert_eq!(c2.conflicts, expected);
    //     assert_eq!(c2.pref, tx1.clone());

    //     // A transaction that spends a distinct input should not conflict.
    //     let tx3 = Tx::new(vec![input3.clone()], vec![output2.clone()]);
    //     dh.insert_tx(tx3.clone()).unwrap();
    //     let expected: HashSet<Tx> = vec![tx3.clone()].iter().cloned().collect();
    //     let c3 = dh.conflicting_txs(tx3.clone()).unwrap();
    //     assert_eq!(c3.conflicts.len(), 1);
    //     assert_eq!(c3.conflicts, expected);
    //     assert_eq!(c3.pref, tx3.clone());

    //     // A transaction that spends multiple conflicting inputs
    //     let output3 = Output::new(pkh2, 800);
    //     let tx4 = Tx::new(vec![input1.clone(), input2.clone(), input3.clone()], vec![output3]);
    //     dh.insert_tx(tx4.clone()).unwrap();
    //     let expected: HashSet<Tx> =
    //         vec![tx1.clone(), tx2.clone(), tx3.clone(), tx4.clone()].iter().cloned().collect();
    //     let c4 = dh.conflicting_txs(tx4.clone()).unwrap();
    //     assert_eq!(c4.conflicts.len(), 4);
    //     assert_eq!(c4.conflicts, expected);
    //     assert_eq!(c4.pref, tx1.clone());
    // }

    // #[actix_rt::test]
    // async fn test_disjoint_inputs() {
    //     let (kp1, kp2, pkh1, pkh2) = generate_keys();

    //     // Some root unspent outputs for `genesis`. We assume this input refers to a UTXO with funds
    //     // but for the purposes of the hypergraph it doesn't matter.
    //     let genesis_tx = Tx::new(
    //         vec![],
    //         vec![
    //             Output::new(pkh1, 1000),
    //             Output::new(pkh2, 1000),
    //             Output::new(pkh2, 500),
    //             Output::new(pkh2, 400),
    //         ],
    //     );

    //     let genesis = UTXOIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs.clone());
    //     let mut dh: UTXOGraph = UTXOGraph::new(genesis.clone());

    //     let input1 = Input::new(&kp1, genesis_tx.hash(), 0);
    //     let input2 = Input::new(&kp2, genesis_tx.hash(), 1);
    //     let input3 = Input::new(&kp2, genesis_tx.hash(), 2);
    //     let input4 = Input::new(&kp2, genesis_tx.hash(), 3);

    //     // A transaction that spends `genesis` and produces a new output for `pkh2`.
    //     let output1 = Output::new(pkh2, 1000);
    //     let tx1 = Tx::new(vec![input1.clone()], vec![output1.clone()]);
    //     dh.insert_tx(tx1.clone()).unwrap();
    //     let expected: HashSet<Tx> = vec![tx1.clone()].iter().cloned().collect();
    //     let c1 = dh.conflicting_txs(tx1.clone()).unwrap();
    //     assert_eq!(c1.conflicts.len(), 1);
    //     assert_eq!(c1.conflicts, expected);
    //     assert_eq!(c1.pref, tx1.clone());

    //     // A transaction that spends some of the same inputs as `tx1`
    //     let output2 = Output::new(pkh2, 900);
    //     let tx2 = Tx::new(vec![input1.clone(), input2.clone()], vec![output2.clone()]);
    //     dh.insert_tx(tx2.clone()).unwrap();
    //     let expected: HashSet<Tx> = vec![tx1.clone(), tx2.clone()].iter().cloned().collect();
    //     let c2 = dh.conflicting_txs(tx2.clone()).unwrap();
    //     assert_eq!(c2.conflicts.len(), 2);
    //     assert_eq!(c2.conflicts, expected);
    //     assert_eq!(c2.pref, tx1.clone());

    //     // A transaction that spends some of the same inputs as `tx2`
    //     let output3 = Output::new(pkh2, 800);
    //     let tx3 =
    //         Tx::new(vec![input2.clone(), input3.clone(), input4.clone()], vec![output3.clone()]);
    //     dh.insert_tx(tx3.clone()).unwrap();
    //     let expected: HashSet<Tx> = vec![tx2.clone(), tx3.clone()].iter().cloned().collect();
    //     let c3 = dh.conflicting_txs(tx3.clone()).unwrap();
    //     assert_eq!(c3.conflicts.len(), 2);
    //     assert_eq!(c3.conflicts, expected);
    //     assert_eq!(c3.pref, tx1.clone());

    //     // A transaction that spends one of the same inputs as `tx3`
    //     let output4 = Output::new(pkh2, 700);
    //     let tx4 = Tx::new(vec![input3.clone()], vec![output4.clone()]);
    //     dh.insert_tx(tx4.clone()).unwrap();
    //     let expected: HashSet<Tx> = vec![tx3.clone(), tx4.clone()].iter().cloned().collect();
    //     let c4 = dh.conflicting_txs(tx4.clone()).unwrap();
    //     assert_eq!(c4.conflicts.len(), 2);
    //     assert_eq!(c4.conflicts, expected);
    //     assert_eq!(c4.pref, tx1.clone());

    //     // Another transaction that spends one of the same inputs as `tx3`
    //     let output5 = Output::new(pkh2, 600);
    //     let tx5 = Tx::new(vec![input4.clone()], vec![output5.clone()]);
    //     dh.insert_tx(tx5.clone()).unwrap();
    //     let expected: HashSet<Tx> = vec![tx3.clone(), tx5.clone()].iter().cloned().collect();
    //     let c5 = dh.conflicting_txs(tx5.clone()).unwrap();
    //     assert_eq!(c5.conflicts.len(), 2);
    //     assert_eq!(c5.conflicts, expected);
    //     assert_eq!(c5.pref, tx1.clone());
    // }

    // #[actix_rt::test]
    // async fn test_outputs() {
    //     let (kp1, kp2, pkh1, pkh2) = generate_keys();

    //     // Some root unspent outputs for `genesis`. We assume this input refers to a UTXO with funds
    //     // but for the purposes of the hypergraph it doesn't matter.
    //     let genesis_tx = Tx::new(vec![], vec![Output::new(pkh1, 1000)]);

    //     let genesis = UTXOIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs.clone());
    //     let mut dh: UTXOGraph = UTXOGraph::new(genesis.clone());

    //     let input1 = Input::new(&kp1, genesis_tx.hash(), 0);

    //     // A transaction that spends `genesis` and produces two new outputs
    //     let tx1 =
    //         Tx::new(vec![input1.clone()], vec![Output::new(pkh1, 1000), Output::new(pkh2, 1000)]);
    //     dh.insert_tx(tx1.clone()).unwrap();
    //     let c1 = dh.conflicting_txs(tx1.clone()).unwrap();
    //     let expected: HashSet<Tx> = vec![tx1.clone()].iter().cloned().collect();
    //     assert_eq!(c1.conflicts.len(), 1);
    //     assert_eq!(c1.conflicts, expected);

    //     // A transaction that spends one output from `tx1` and produces a new output.
    //     let input2 = Input::new(&kp1, tx1.hash(), 0);
    //     let tx2 = Tx::new(vec![input2.clone()], vec![Output::new(pkh1, 1000)]);
    //     dh.insert_tx(tx2.clone()).unwrap();
    //     let c2 = dh.conflicting_txs(tx2.clone()).unwrap();
    //     let expected: HashSet<Tx> = vec![tx2.clone()].iter().cloned().collect();
    //     assert_eq!(c2.conflicts.len(), 1);
    //     assert_eq!(c2.conflicts, expected);

    //     // A transaction that spends another output from `tx1` and produces two new outputs.
    //     let input3 = Input::new(&kp2, tx1.hash(), 1);
    //     let tx3 =
    //         Tx::new(vec![input3.clone()], vec![Output::new(pkh1, 1000), Output::new(pkh2, 1000)]);
    //     dh.insert_tx(tx3.clone()).unwrap();
    //     let c3 = dh.conflicting_txs(tx3.clone()).unwrap();
    //     let expected: HashSet<Tx> = vec![tx3.clone()].iter().cloned().collect();
    //     assert_eq!(c3.conflicts.len(), 1);
    //     assert_eq!(c3.conflicts, expected);

    //     // A transaction which spends tx3 outputs and conflicts with tx1
    //     let input4 = Input::new(&kp1, tx3.hash(), 0);
    //     let tx4 = Tx::new(vec![input1.clone(), input4.clone()], vec![Output::new(pkh1, 1000)]);
    //     dh.insert_tx(tx4.clone()).unwrap();
    //     let c4 = dh.conflicting_txs(tx4.clone()).unwrap();
    //     let expected: HashSet<Tx> = vec![tx1.clone(), tx4.clone()].iter().cloned().collect();
    //     assert_eq!(c4.conflicts.len(), 2);
    //     assert_eq!(c4.conflicts, expected);
    //     assert_eq!(c4.pref, tx1.clone());
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

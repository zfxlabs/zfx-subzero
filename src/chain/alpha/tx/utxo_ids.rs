use super::input::{Input, UTXOId};
use super::inputs::Inputs;
use super::output::Output;
use super::outputs::Outputs;
use super::tx::{Tx, TxHash};

use std::collections::HashSet;

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UTXOIds {
    pub inner: HashSet<UTXOId>,
}

impl std::iter::FromIterator<[u8; 32]> for UTXOIds {
    fn from_iter<I: IntoIterator<Item = [u8; 32]>>(iter: I) -> Self {
        let mut hs = HashSet::new();
        for i in iter {
            hs.insert(i);
        }
        UTXOIds { inner: hs }
    }
}

impl std::ops::Deref for UTXOIds {
    type Target = HashSet<UTXOId>;

    fn deref(&self) -> &'_ Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for UTXOIds {
    fn deref_mut(&mut self) -> &'_ mut Self::Target {
        &mut self.inner
    }
}

impl Hash for UTXOIds {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut v: Vec<UTXOId> = self.iter().cloned().collect();
        v.sort();
        v.hash(state);
    }
}

impl std::cmp::Eq for UTXOIds {}

impl std::cmp::PartialEq for UTXOIds {
    fn eq(&self, other: &Self) -> bool {
        let mut self_v: Vec<UTXOId> = self.iter().cloned().collect();
        let mut other_v: Vec<UTXOId> = other.iter().cloned().collect();
        self_v.sort();
        other_v.sort();
        self_v == other_v
    }
}

impl std::cmp::Ord for UTXOIds {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut self_v: Vec<UTXOId> = self.iter().cloned().collect();
        let mut other_v: Vec<UTXOId> = other.iter().cloned().collect();
        self_v.sort();
        other_v.sort();
        self_v.cmp(&other_v)
    }
}

impl std::cmp::PartialOrd for UTXOIds {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let mut self_v: Vec<UTXOId> = self.iter().cloned().collect();
        let mut other_v: Vec<UTXOId> = other.iter().cloned().collect();
        self_v.sort();
        other_v.sort();
        Some(self_v.cmp(&other_v))
    }
}

impl UTXOIds {
    pub fn new(hs: HashSet<UTXOId>) -> Self {
        UTXOIds { inner: hs }
    }

    pub fn empty() -> Self {
        UTXOIds { inner: HashSet::new() }
    }

    pub fn from_inputs(inputs: Inputs<Input>) -> Self {
        let mut utxo_ids = HashSet::new();
        for input in inputs.iter() {
            utxo_ids.insert(input.utxo_id());
        }
        UTXOIds { inner: utxo_ids }
    }

    pub fn from_outputs(txhash: TxHash, outputs: Outputs<Output>) -> Self {
        let mut utxo_ids = HashSet::new();
        for i in 0..outputs.len() {
            let source = txhash.clone();
            let i = i.clone() as u8;
            let bytes = vec![source.to_vec(), vec![i]].concat();
            let encoded = bincode::serialize(&bytes).unwrap();
            let utxo_id = blake3::hash(&encoded).as_bytes().clone();
            utxo_ids.insert(utxo_id);
        }
        UTXOIds { inner: utxo_ids }
    }

    #[inline]
    pub fn intersects_with(&self, other: &UTXOIds) -> bool {
        !self.is_disjoint(other)
    }

    pub fn intersect(&self, other: &UTXOIds) -> UTXOIds {
        UTXOIds { inner: self.intersection(other).cloned().collect() }
    }

    pub fn left_difference(&mut self, other: &UTXOIds) -> UTXOIds {
        UTXOIds { inner: self.difference(other).cloned().collect() }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use ed25519_dalek::Keypair;

    #[actix_rt::test]
    async fn test_utxo_ids() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        let genesis_tx =
            Tx::from_vecs(vec![], vec![Output::new(pkh1, 1000), Output::new(pkh2, 1000)]);
        let genesis_output_utxo_ids =
            UTXOIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs.clone());
        let input1 = Input::new(&kp1, genesis_tx.hash(), 0);
        let input2 = Input::new(&kp2, genesis_tx.hash(), 1);
        let output1 = Output::new(pkh1, 1000);
        let tx1 = Tx::from_vecs(vec![input1, input2], vec![output1]);
        let tx1_input_utxo_ids = UTXOIds::from_inputs(tx1.inputs.clone());
        assert_eq!(genesis_output_utxo_ids.clone(), tx1_input_utxo_ids.clone());
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

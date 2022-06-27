use super::cell_id::CellId;
use super::inputs::Inputs;
use super::outputs::Outputs;
use super::types::CellHash;
use super::Result;

use std::collections::HashSet;

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

/// Defines an id for the whole [Cell][crate::cell::Cell] by combining all [CellId]s for each of its [Output].
#[derive(Clone, Serialize, Deserialize)]
pub struct CellIds {
    pub inner: HashSet<CellId>,
}

impl std::fmt::Debug for CellIds {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        let mut s = "\n".to_owned();
        for cell_id in self.iter() {
            s = format!("{}{:?}\n", s, cell_id);
        }
        write!(fmt, "{}", s)
    }
}

impl std::iter::FromIterator<[u8; 32]> for CellIds {
    fn from_iter<I: IntoIterator<Item = [u8; 32]>>(iter: I) -> Self {
        let mut hs = HashSet::new();
        for bytes in iter {
            hs.insert(CellId::new(bytes));
        }
        CellIds { inner: hs }
    }
}

impl std::iter::FromIterator<CellId> for CellIds {
    fn from_iter<I: IntoIterator<Item = CellId>>(iter: I) -> Self {
        let mut hs = HashSet::new();
        for cell_id in iter {
            hs.insert(cell_id.clone());
        }
        CellIds { inner: hs }
    }
}

impl std::ops::Deref for CellIds {
    type Target = HashSet<CellId>;

    fn deref(&self) -> &'_ Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for CellIds {
    fn deref_mut(&mut self) -> &'_ mut Self::Target {
        &mut self.inner
    }
}

impl Hash for CellIds {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut v: Vec<CellId> = self.iter().cloned().collect();
        v.sort();
        v.hash(state);
    }
}

impl std::cmp::Eq for CellIds {}

impl std::cmp::PartialEq for CellIds {
    fn eq(&self, other: &Self) -> bool {
        let mut self_v: Vec<CellId> = self.iter().cloned().collect();
        let mut other_v: Vec<CellId> = other.iter().cloned().collect();
        self_v.sort();
        other_v.sort();
        self_v == other_v
    }
}

impl std::cmp::Ord for CellIds {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut self_v: Vec<CellId> = self.iter().cloned().collect();
        let mut other_v: Vec<CellId> = other.iter().cloned().collect();
        self_v.sort();
        other_v.sort();
        self_v.cmp(&other_v)
    }
}

impl std::cmp::PartialOrd for CellIds {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let mut self_v: Vec<CellId> = self.iter().cloned().collect();
        let mut other_v: Vec<CellId> = other.iter().cloned().collect();
        self_v.sort();
        other_v.sort();
        Some(self_v.cmp(&other_v))
    }
}

impl CellIds {
    /// Create new instance from a set of [CellId]s.
    ///
    /// ## Parameters
    /// * `hs` - set of [CellId]s for joining to a single id
    pub fn new(hs: HashSet<CellId>) -> Self {
        CellIds { inner: hs }
    }

    /// Create an instance with no [CellId]s.
    pub fn empty() -> Self {
        CellIds { inner: HashSet::new() }
    }

    /// Create an instance from [Inputs] of a [Cell][crate::cell::Cell].
    /// For each input, the function [OutputIndex::cell_id] is called in order
    /// to compose an Id and assign it to the final instance.
    ///
    /// ## Parameters
    /// * `inputs` - inputs of a [Cell][crate::cell::Cell]
    pub fn from_inputs(inputs: Inputs) -> Result<Self> {
        let mut cell_ids = HashSet::new();
        for input in inputs.iter() {
            cell_ids.insert(input.cell_id()?);
        }
        Ok(CellIds { inner: cell_ids })
    }

    /// Create an instance from a [CellHash] and [Outputs] of the [Cell][crate::cell::Cell].
    /// For each output, the function [CellId::from_output] is called in order
    /// to compose an Id and assign it to the final instance.
    ///
    /// ## Parameters
    /// * `cell_hash` - hash of a [Cell][crate::cell::Cell]
    /// * `outputs` - outputs of a [Cell][crate::cell::Cell]
    pub fn from_outputs(cell_hash: CellHash, outputs: Outputs) -> Result<Self> {
        let mut cell_ids = HashSet::new();
        for i in 0..outputs.len() {
            cell_ids.insert(CellId::from_output(cell_hash.clone(), i as u8, outputs[i].clone())?);
        }
        Ok(CellIds { inner: cell_ids })
    }

    /// Returns `true` if `self` has no elements in common with `other`.
    #[inline]
    pub fn intersects_with(&self, other: &CellIds) -> bool {
        !self.is_disjoint(other)
    }

    /// Returns a new [CellIds] having values presented in `self` and `other`.
    pub fn intersect(&self, other: &CellIds) -> CellIds {
        CellIds { inner: self.intersection(other).cloned().collect() }
    }

    /// Returns a new [CellIds] having values presented in `self` but not in `other`.
    pub fn left_difference(&self, other: &CellIds) -> CellIds {
        CellIds { inner: self.difference(other).cloned().collect() }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::alpha::coinbase::CoinbaseOperation;
    use crate::alpha::transfer::TransferOperation;
    use crate::cell::Cell;

    use std::convert::TryInto;

    use ed25519_dalek::Keypair;

    #[actix_rt::test]
    async fn test_cell_ids() {
        let (kp1, _kp2, pkh1, pkh2) = generate_keys();

        let genesis_op = CoinbaseOperation::new(vec![(pkh1.clone(), 1000), (pkh1.clone(), 1000)]);
        let genesis_tx: Cell = genesis_op.try_into().unwrap();
        let genesis_output_cell_ids =
            CellIds::from_outputs(genesis_tx.hash(), genesis_tx.outputs()).unwrap();

        let transfer_op = TransferOperation::new(genesis_tx, pkh2.clone(), pkh1.clone(), 1100);
        let transfer_tx = transfer_op.transfer(&kp1).unwrap();
        let transfer_tx_input_cell_ids = CellIds::from_inputs(transfer_tx.inputs()).unwrap();
        assert_eq!(genesis_output_cell_ids.clone(), transfer_tx_input_cell_ids.clone());
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

use super::cell_id::CellId;
use super::types::*;
use super::Result;

/// A reference to [Output] contained in a [Cell].
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct OutputIndex {
    /// hash of a [Cell] being spent
    pub cell_hash: CellHash,
    /// position of [Output] in the list of [Outputs] in [Cell]
    pub index: u8,
}

impl std::fmt::Debug for OutputIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "<{}:{}>", hex::encode(self.cell_hash), self.index)
    }
}

impl OutputIndex {
    /// Create instance of Output Index.
    ///
    /// ## Parameters
    /// * `cell_hash` - hash of a [Cell] being spent.
    /// * `index` - position of [Output] in the list of [Outputs] in [Cell].
    pub fn new(cell_hash: CellHash, index: u8) -> Self {
        OutputIndex { cell_hash, index }
    }

    /// Returns an id of cell, composed of serialized [Cell] hash and index
    /// _(position of [Output] in the list of [Outputs] in [Cell])_.
    pub fn cell_id(&self) -> Result<CellId> {
        let bytes = vec![self.cell_hash.clone().to_vec(), vec![self.index]].concat();
        let encoded = bincode::serialize(&bytes)?;
        Ok(CellId::new(blake3::hash(&encoded).as_bytes().clone()))
    }
}

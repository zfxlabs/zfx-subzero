use super::cell_id::CellId;
use super::types::*;
use super::Result;

/// A reference to a the output contained in a cell.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct OutputIndex {
    pub cell_hash: CellHash,
    pub index: u8,
}

impl OutputIndex {
    pub fn new(cell_hash: CellHash, index: u8) -> Self {
        OutputIndex { cell_hash, index }
    }

    pub fn cell_id(&self) -> Result<CellId> {
        let bytes = vec![self.cell_hash.clone().to_vec(), vec![self.index]].concat();
        let encoded = bincode::serialize(&bytes)?;
        Ok(CellId::new(blake3::hash(&encoded).as_bytes().clone()))
    }
}

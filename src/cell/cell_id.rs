use super::outputs::Output;
use super::types::CellHash;
use super::Result;

use std::ops::{Deref, DerefMut};

// The hash of a cells output index.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct CellId([u8; 32]);

impl std::fmt::Debug for CellId {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        write!(fmt, "{}", hex::encode(self.0))
    }
}

impl Deref for CellId {
    type Target = [u8; 32];

    fn deref(&self) -> &'_ Self::Target {
        &self.0
    }
}

impl DerefMut for CellId {
    fn deref_mut(&mut self) -> &'_ mut Self::Target {
        &mut self.0
    }
}

impl Into<[u8; 32]> for CellId {
    fn into(self) -> [u8; 32] {
        self.0
    }
}

impl CellId {
    pub fn new(cell_id: [u8; 32]) -> Self {
        CellId(cell_id)
    }

    pub fn from_output(cell_hash: CellHash, i: u8, output: Output) -> Result<Self> {
        let bytes = vec![cell_hash.to_vec(), vec![i]].concat();
        let encoded = bincode::serialize(&bytes)?;
        Ok(CellId(blake3::hash(&encoded).as_bytes().clone()))
    }
}

use super::outputs::Output;
use super::types::CellHash;
use super::Result;

use std::ops::{Deref, DerefMut};

use crate::colored::Colorize;

/// An unique id of a [Cell], which is usually derived from serialization result
/// of a hash of the cell and a position of [Output] in [Outputs] list of the cell.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct CellId([u8; 32]);

impl std::fmt::Debug for CellId {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        write!(fmt, "{}", hex::encode(self.0))
    }
}

impl std::fmt::Display for CellId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = format!("{}", hex::encode(self.0)).blue();
        write!(f, "{}", s)
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
    /// Create an instance of CellId from serialized data.
    ///
    /// ## Parameters
    /// * `cell_id` - serialized data (ex. combination of cell hash and output position,
    /// see. [CellId::from_output])
    pub fn new(cell_id: [u8; 32]) -> Self {
        CellId(cell_id)
    }

    /// Create an instance of CellId from a hash of [Cell] and
    /// position of [Output] in [Outputs] list of the [Cell].
    ///
    /// ## Parameters
    /// * `cell_hash` - hash of [Cell]
    /// * `i` - position of [Output] in [Outputs] list of the [Cell]
    // TODO check if we need the `output` argument
    pub fn from_output(cell_hash: CellHash, i: u8, _output: Output) -> Result<Self> {
        let bytes = vec![cell_hash.to_vec(), vec![i]].concat();
        let encoded = bincode::serialize(&bytes)?;
        Ok(CellId(blake3::hash(&encoded).as_bytes().clone()))
    }
}

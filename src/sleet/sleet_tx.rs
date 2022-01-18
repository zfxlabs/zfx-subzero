use crate::cell::types::CellHash;
use crate::cell::Cell;

/// The `SleetTx` is a consensus specific representation of a transaction, containing a
/// chain specific transaction as its `inner` field.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SleetTx {
    pub parents: Vec<CellHash>,
    pub inner: Cell,
}

impl SleetTx {
    pub fn new(parents: Vec<CellHash>, inner: Cell) -> Self {
        SleetTx { parents, inner }
    }

    /// Returns the hash of the inner transaction.
    pub fn hash(&self) -> [u8; 32] {
        self.inner.hash()
    }
}

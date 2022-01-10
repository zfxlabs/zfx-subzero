use crate::chain::alpha::{Transaction, TxHash};

/// The `SleetTx` is a consensus specific representation of a transaction, containing a
/// chain specific transaction as its `inner` field.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SleetTx {
    pub parents: Vec<TxHash>,
    pub inner: Transaction,
}

impl SleetTx {
    pub fn new(parents: Vec<TxHash>, inner: Transaction) -> Self {
        SleetTx { parents, inner }
    }

    /// Returns the hash of the inner transaction.
    pub fn hash(&self) -> [u8; 32] {
        self.inner.hash()
    }
}

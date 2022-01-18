use crate::chain::alpha::{Transaction, TxHash};
use crate::colored::Colorize;

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

impl std::fmt::Display for SleetTx {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = format!("{}", self.inner);
        let mut ps = "".to_owned();
        for p in self.parents.iter() {
            let h = hex::encode(p);
            ps.push(' ');
            ps.push_str(&h);
        }
        let s = format!("{}[{}]{}\n", s, "parents".yellow(), ps);
        write!(f, "{}", s)
    }
}

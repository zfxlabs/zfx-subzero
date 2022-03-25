use crate::alpha::types::TxHash;
use crate::cell::Cell;

use crate::colored::Colorize;

/// Status of the transaction
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum TxStatus {
    /// New transaction
    Pending,
    Queried,
    Accepted,
    Rejected,
    /// Removed progeny of a rejected transaction
    Removed,
}

/// The `Tx` is a consensus specific representation of a transaction, containing a
/// chain specific transaction as its `cell` field.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Tx {
    pub parents: Vec<TxHash>,
    pub cell: Cell,
    pub status: TxStatus,
}

impl Tx {
    pub fn new(parents: Vec<TxHash>, cell: Cell) -> Self {
        Tx { parents, cell, status: TxStatus::Pending }
    }

    /// Returns the hash of the inner cell.
    /// Note, that we rely on the fact that both `CellHash` and `TxHash` are type synonyms for `[u8; 32]`
    pub fn hash(&self) -> TxHash {
        self.cell.hash()
    }
}

impl std::fmt::Display for Tx {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = format!("{}", self.cell);
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

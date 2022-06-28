//! [Tx] represents a transaction in [`sleet`][crate::sleet]
use crate::alpha::types::TxHash;
use crate::cell::Cell;

use crate::colored::Colorize;

/// Status of the transaction
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum TxStatus {
    /// New transaction
    Pending,
    /// Transaction was queried
    Queried,
    /// Transaction was accepted as final
    Accepted,
    /// Transaction rejected as it conflicted with an accepted transaction
    Rejected,
    /// Removed progeny of a rejected transaction
    Removed,
}

/// The `Tx` is a consensus specific representation of a transaction, containing a
/// chain specific transaction as its `cell` field, and its parents in the Sleet [DAG][crate::graph::DAG] in its `parents` field.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Tx {
    /// Parents of the transaction represented in DAG
    pub parents: Vec<TxHash>,
    /// UTXO data containing information about transferred, staked or remaining balances
    /// for accounts
    pub cell: Cell,
    /// Transaction status
    pub status: TxStatus,
}

impl Tx {
    /// Create new transaction with [TxStatus::Pending] status.
    ///
    /// * `parents` - a list of parent transactions, represented in [hash][Tx::hash].
    /// Parent transactions can be obtained, for example, from the DAG of [Sleet][crate::sleet]
    /// to form a strong connection between new transaction and parent ones.
    /// * `cell` - a cell to enclose in this transaction
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
        let s = format!("{}[{}] {:?}\n", s, "status".yellow(), self.status);
        write!(f, "{}", s)
    }
}

use super::{Input, Inputs, Output, Outputs, PublicKeyHash, Tx};

use crate::colored::Colorize;

/// Coinbase transactions are used for block rewards and initial staking allocations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoinbaseTx {
    /// The inputs / outputs of this transaction.
    pub tx: Tx,
}

impl std::fmt::Display for CoinbaseTx {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let h = hex::encode(self.hash());
        let s = format!("[{}] {}\n", "tx_hash".yellow(), h);
        let s = format!("{}[{}] Coinbase\n", s, "type".yellow());
        let s = format!("{}[{}] {}\n", s, "spendable".yellow(), self.tx.sum());
        write!(f, "{}", s)
    }
}

impl CoinbaseTx {
    pub fn new(outputs: Outputs<Output>) -> Self {
        CoinbaseTx { tx: Tx::new(Inputs::new(vec![]), outputs) }
    }

    pub fn from_output(owner: PublicKeyHash, value: u64) -> Self {
        let output = Output::new(owner, value.clone());
        CoinbaseTx { tx: Tx::from_vecs(vec![], vec![output]) }
    }

    pub fn inputs(&self) -> Inputs<Input> {
        self.tx.inputs()
    }

    pub fn outputs(&self) -> Outputs<Output> {
        self.tx.outputs()
    }

    pub fn hash(&self) -> [u8; 32] {
        let encoded = bincode::serialize(self).unwrap();
        blake3::hash(&encoded).as_bytes().clone()
    }
}

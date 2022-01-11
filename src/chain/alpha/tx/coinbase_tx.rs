use super::{Input, Output, PublicKeyHash, Tx};

/// Coinbase transactions are used for block rewards and initial staking allocations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoinbaseTx {
    /// The inputs / outputs of this transaction.
    pub tx: Tx,
}

impl CoinbaseTx {
    /// Creates a new coinbase transaction.
    pub fn new(owner: PublicKeyHash, value: u64) -> Self {
        let output = Output::new(owner, value.clone());
        CoinbaseTx { tx: Tx::new(vec![], vec![output]) }
    }

    pub fn inputs(&self) -> Vec<Input> {
        self.tx.inputs.clone()
    }

    pub fn outputs(&self) -> Vec<Output> {
        self.tx.outputs.clone()
    }

    pub fn hash(&self) -> [u8; 32] {
        let encoded = bincode::serialize(self).unwrap();
        blake3::hash(&encoded).as_bytes().clone()
    }
}

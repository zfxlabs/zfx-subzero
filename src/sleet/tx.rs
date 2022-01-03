use crate::chain::alpha::Transaction;

// Consensus representation of a transaction

pub type TxHash = [u8; 32];

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Tx {
    pub parents: Vec<TxHash>,
    pub inner: Transaction,
}

impl Tx {
    pub fn new(parents: Vec<TxHash>, inner: Transaction) -> Self {
	Tx { parents, inner }
    }

    pub fn hash(&self) -> [u8; 32] {
	let encoded = bincode::serialize(self).unwrap();
	blake3::hash(&encoded).as_bytes().clone()
    }
}

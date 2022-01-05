use crate::chain::alpha;

/// The `SleetTx` is a consensus specific representation of a transaction, containing a
/// chain specific transaction as its `inner` field.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SleetTx {
    pub parents: Vec<alpha::TxHash>,
    pub inner: alpha::Tx,
}

impl SleetTx {
    pub fn new(parents: Vec<alpha::TxHash>, inner: alpha::Tx) -> Self {
	SleetTx { parents, inner }
    }

    pub fn hash(&self) -> [u8; 32] {
	let encoded = bincode::serialize(self).unwrap();
	blake3::hash(&encoded).as_bytes().clone()
    }
}

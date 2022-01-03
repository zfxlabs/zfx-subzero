
pub type TxHash = [u8; 32];

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Tx {
    pub parents: Vec<TxHash>,
    pub data: Vec<u8>,
}

impl Tx {
    pub fn new(parents: Vec<TxHash>, data: Vec<u8>) -> Self {
	Tx { parents, data }
    }

    pub fn hash(&self) -> [u8; 32] {
	let encoded = bincode::serialize(self).unwrap();
	blake3::hash(&encoded).as_bytes().clone()
    }
}

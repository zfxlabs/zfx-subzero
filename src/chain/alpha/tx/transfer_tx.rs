use super::{Tx, TxHash, Input, Output, PublicKeyHash};

use ed25519_dalek::Keypair;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferTx {
    /// The inputs / outputs of this transaction.
    pub tx: Tx,
}

impl TransferTx {
    /// Creates a new transfer transaction. Transfer transactions are used to send tokens
    /// from the owner of `keypair` to some destination public key hash.
    pub fn new(keypair: &Keypair, tx: Tx, destination: PublicKeyHash, change: PublicKeyHash, value: u64) -> Self {
	let tx = tx.spend(keypair, destination, change, value).unwrap();
	TransferTx { tx }
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
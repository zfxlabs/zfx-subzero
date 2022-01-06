use zfx_id::Id;

use super::{Tx, TxHash, Input, Output, Transaction};

use ed25519_dalek::Keypair;
use tai64::Tai64;

// A transaction is constructed from inputs and outputs and has a type, which we use to
// create special types of transactions. Note: This is for testing only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StakeTx {
    pub node_id: Id,
    // pub start_time: Tai64,
    // pub end_time: Tai64,
    /// The inputs / outputs of this transaction.
    pub tx: Tx,
}

impl StakeTx {

    /// Creates a new signed staking transaction.
    pub fn new(keypair: &Keypair, node_id: Id, source: TxHash, i: u8, value: u64) -> Self {
	let encoded = bincode::serialize(&keypair.public).unwrap();
	let pkh = blake3::hash(&encoded).as_bytes().clone();
	let input = Input::new(keypair, source, i);
	let output = Output::new(pkh, value.clone());
	StakeTx { node_id, tx: Tx::new(vec![input], vec![output]) }
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

#[cfg(test)]
mod test {
    use super::*;

    #[actix_rt::test]
    async fn test_stake() { }
}

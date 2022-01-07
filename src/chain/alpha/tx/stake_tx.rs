use crate::chain::alpha::Amount;

use super::{Tx, TxHash, Input, Output, Transaction};

use zfx_id::Id;
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
    /// The amount that the staker wishes to lock for staking.
    pub value: Amount,
}

impl StakeTx {

    /// Creates a new signed staking transaction.
    pub fn new(keypair: &Keypair, node_id: Id, tx: Tx, value: Amount) -> Self {
	let encoded = bincode::serialize(&keypair.public).unwrap();
	let pkh = blake3::hash(&encoded).as_bytes().clone();
	// Spend the supplied transaction (to self)
	let tx = tx.stake(keypair, pkh.clone(), pkh.clone(), value.clone()).unwrap();
	StakeTx { node_id, tx, value }
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

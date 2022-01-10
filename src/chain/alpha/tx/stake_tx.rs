use crate::colored::Colorize;

use crate::chain::alpha::Amount;
use crate::zfx_id::Id;

use super::{Input, Output, Transaction, Tx, TxHash};

use ed25519_dalek::Keypair;
use tai64::Tai64;

// A transaction is constructed from inputs and outputs and has a type, which we use to
// create special types of transactions. Note: This is for testing only.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StakeTx {
    pub node_id: Id,
    // pub start_time: Tai64,
    // pub end_time: Tai64,
    /// The inputs / outputs of this transaction.
    pub tx: Tx,
    /// The amount that the staker wishes to lock for staking.
    pub value: Amount,
}

impl std::fmt::Debug for StakeTx {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let h = hex::encode(self.hash());
        let s = format!("[{}] {}\n", "node_id".yellow(), self.node_id);
        let s = format!("{}[{}] {}\n", s, "tx_hash".yellow(), h);
        let s = format!("{}[{}] {}\n", s, "staked".yellow(), self.value);
        let s = format!("{}[{}] {}\n", s, "spendable".yellow(), self.tx.sum());
        write!(f, "{}", s)
    }
}

impl StakeTx {
    /// Creates a new signed staking transaction.
    pub fn new(keypair: &Keypair, node_id: Id, tx: Tx, value: Amount) -> Self {
        let encoded = bincode::serialize(&keypair.public).unwrap();
        let pkh = blake3::hash(&encoded).as_bytes().clone();
        // Spend the supplied transaction (to self)
        let tx = tx.stake(keypair, pkh.clone(), value.clone()).unwrap();
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
    async fn test_stake() {}
}

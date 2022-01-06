use super::{Input, Output};

use super::stake_tx::StakeTx;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Transaction {
    StakeTx(StakeTx),
}

impl Transaction {
    pub fn inputs(&self) -> Vec<Input> {
	match self {
	    Transaction::StakeTx(stake_tx) =>
		stake_tx.inputs(),
	}
    }

    pub fn outputs(&self) -> Vec<Output> {
	match self {
	    Transaction::StakeTx(stake_tx) =>
		stake_tx.outputs(),
	}
    }

    pub fn hash(&self) -> [u8; 32] {
	match self {
	    Transaction::StakeTx(stake_tx) =>
		stake_tx.hash(),
	}
    }
}

use super::{Input, Output};

use super::coinbase_tx::CoinbaseTx;
use super::stake_tx::StakeTx;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Transaction {
    CoinbaseTx(CoinbaseTx),
    StakeTx(StakeTx),
}

impl Transaction {
    pub fn is_coinbase(&self) -> bool {
	match self {
	    Transaction::CoinbaseTx(_) =>
		true,
	    _ =>
		false,
	}
    }

    pub fn inputs(&self) -> Vec<Input> {
	match self {
	    // coinbase transactions have no inputs
	    Transaction::CoinbaseTx(tx) =>
		vec![],
	    Transaction::StakeTx(tx) =>
		tx.inputs(),
	}
    }

    pub fn outputs(&self) -> Vec<Output> {
	match self {
	    Transaction::CoinbaseTx(tx) =>
		tx.outputs(),
	    Transaction::StakeTx(tx) =>
		tx.outputs(),
	}
    }

    pub fn hash(&self) -> [u8; 32] {
	match self {
	    Transaction::CoinbaseTx(tx) =>
		tx.hash(),
	    Transaction::StakeTx(tx) =>
		tx.hash(),
	}
    }
}

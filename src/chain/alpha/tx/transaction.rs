use super::{Input, Output};

use super::coinbase_tx::CoinbaseTx;
use super::stake_tx::StakeTx;
use super::transfer_tx::TransferTx;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Transaction {
    CoinbaseTx(CoinbaseTx),
    StakeTx(StakeTx),
    TransferTx(TransferTx),
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
	    Transaction::TransferTx(tx) =>
		tx.inputs(),
	}
    }

    pub fn outputs(&self) -> Vec<Output> {
	match self {
	    Transaction::CoinbaseTx(tx) =>
		tx.outputs(),
	    Transaction::StakeTx(tx) =>
		tx.outputs(),
	    Transaction::TransferTx(tx) =>
		tx.outputs(),
	}
    }

    pub fn hash(&self) -> [u8; 32] {
	match self {
	    Transaction::CoinbaseTx(tx) =>
		tx.hash(),
	    Transaction::StakeTx(tx) =>
		tx.hash(),
	    Transaction::TransferTx(tx) =>
		tx.hash(),
	}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::zfx_id::Id;
    use rand::{CryptoRng, rngs::OsRng};
    use ed25519_dalek::Keypair;

    fn hash_public(keypair: &Keypair) -> [u8; 32] {
	let enc = bincode::serialize(&keypair.public).unwrap();
	blake3::hash(&enc).as_bytes().clone()
    }

    #[actix_rt::test]
    async fn test_staking_allocation() {
	let mut csprng = OsRng{};
	let kp = Keypair::generate(&mut csprng);
	let pkh = hash_public(&kp);
	let node_id = Id::generate();
	let fee = 100;
	let allocation = 2000;
	let staked = 1000;
	let alloc_tx = CoinbaseTx::new(pkh, allocation.clone());
	let stake_tx = StakeTx::new(&kp, node_id, alloc_tx.tx, staked);
	assert_eq!(stake_tx.inputs().len(), 1);
	assert_eq!(stake_tx.outputs().len(), 1);
	assert_eq!(stake_tx.tx.sum(), staked - fee);
    }

}

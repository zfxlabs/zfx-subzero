use zfx_id::Id;

use tai64::Tai64;

// A transaction is constructed from inputs and outputs and has a type, which we use to
// create special types of transactions. Note: This is for testing only.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StakeTx {
    pub node_id: Id,
    // pub start_time: Tai64,
    // pub end_time: Tai64,
    pub qty: u64,
}

impl StakeTx {
    /// Coinbase stake transaction (at genesis)
    pub fn new(node_id: Id, qty: u64) -> Self {
	StakeTx { node_id, qty }
    }

    // Regular stake transaction (after genesis)
    // pub fn stake(pkh, node_id, start_time, end_time, utxos: Vec<Output>) {}
}

#[cfg(test)]
mod test {
    use super::*;

    // #[actix_rt::test]
    // async fn test_spend() {
    // 	// let (pk, sk) = Keypair::new();
    // 	// let pkh = hash(pk);

    // 	let utxo1 = Transaction::coinbase(pkh1, 1000);
    // 	let utxo2 = Transaction::coinbase(pkh2, 1000);
    // 	let utxo3 = Transaction::coinbase(pkh3, 1000);
	
    // }
}

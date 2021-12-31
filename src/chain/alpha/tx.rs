use zfx_id::Id;

use tai64::Tai64;

// // A UTXO is an unspent transaction output, spendable by some public key
// pub struct Output {
//     pkh: Id,
//     qty: u64,
// }

// impl Output {
//     fn new(pkh: Id, qty: u64) -> Output {
// 	Output { pkh, qty }
//     }
// }

// // An input spends a UTXO based on the signature contained therein, we use an index here
// // instead of a signature for testing purposes.
// pub struct Input {
//     i: u32,
//     // signature: Signature,
// }

// // A transaction is constructed from inputs and outputs and has a type, which we use to
// // create special types of transactions.
// pub struct Tx {
//     inputs: Vec<Input>,
//     outputs: Vec<Output>,
// }

// impl Tx {
//     pub fn new(inputs: Vec<Input>, outputs: Vec<Output>) -> Tx {
// 	Tx { inputs, outputs }
//     }

//     /// A coinbase transaction is a transaction with only an output defined.
//     pub fn coinbase(pkh: Id, qty: u64) {
// 	let output = Output::new(pkh, qty);
// 	Tx::new(vec![], vec![output])
//     }

//     pub fn spend(&self, qty: u64) -> Tx {
// 	let total = self.outputs.clone().fold(0u64, |tot, out| tot + out.qty);
// 	if qty > total {
// 	    // error: insufficient balance
// 	}
// 	// if total - qty < fee {
// 	    // error: insufficient balance to pay the transaction fee
// 	// }
// 	let inputs = self.outputs.fold(vec![], |is, out| {
	    
// 	});
//     }
// }

// A transaction is constructed from inputs and outputs and has a type, which we use to
// create special types of transactions.
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

// #[cfg(test)]
// mod tests {
//     use super::*;

//     use crate::util;
    
//     use std::net::SocketAddr;

//     #[actix_rt::test]
//     async fn test_stake_tx() {
// 	let node_id = util::id_from_ip("127.0.0.1:1234".parse().unwrap());
// 	let start_time = Tai64::now();
// 	let end_time = Tai64::now().add(60 * 3);
// 	let qty = 10000;
// 	let _ = StakeTx::new(node_id, start_time, end_time, qty);
//     }
// }

use super::{Result, Error};
use super::input::Input;
use super::output::{Output, PublicKeyHash, Amount};

use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signature, Signer};

pub type TxHash = [u8; 32];

pub const fee: u64 = 100;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Tx {
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
}

impl Tx {
    pub fn new(inputs: Vec<Input>, outputs: Vec<Output>) -> Tx {
	Tx { inputs, outputs }
    }

    pub fn coinbase(owner: PublicKeyHash, value: Amount) -> Tx {
	Tx::new(vec![], vec![Output::new(owner, value)])
    }

    pub fn spend(&self, keypair: &Keypair, destination: PublicKeyHash, change: PublicKeyHash, value: Amount) -> Result<Tx> {
	let owner = keypair.public.clone();

	// Sum output amounts
	let mut total = 0;
	for output in self.outputs.iter() {
	    total += output.value;
	}
	if value == 0 {
	    return Err(Error::ZeroSpend);
	}
	if value >= total {
	    return Err(Error::ExceedsAvailableFunds);
	}
	if value > total - fee {
	    return Err(Error::ExceedsAvailableFunds);
	}

	let tx_hash = self.hash();

	// Consume outputs and construct inputs, remaining inputs should be reflected in
	// the change amount.
	let mut i = 0;
	let mut amount_left = value.clone();
	let mut change_amount = 0;
	let mut consumed = 0;
	let mut inputs = vec![];
	for output in self.outputs.iter() {
	    if consumed < value.clone() {
		let output_hash = output.hash();
		let signature = keypair.sign(&output_hash);
		let input = Input {
		    source: tx_hash.clone(),
		    i: i.clone(),
		    owner: owner.clone(),
		    signature,
		};
		inputs.push(input);
		if consumed + output.value > value.clone() {
		    consumed = value - fee;
		} else {
		    consumed += output.value;
		}
		if output.value > amount_left {
		    change_amount = output.value - amount_left;
		    amount_left = 0;
		} else {
		    amount_left -= output.value;
		}
		i += 1;
	    } else {
		break;
	    }
	}

	// Aggregate the spent value into one main output.
	let main_output = Output::new(destination.clone(), consumed.clone());
	// Create a change output.
	let outputs = if amount_left > 0 {
	    vec![main_output, Output::new(change.clone(), change_amount.clone())]
	} else {
	    vec![main_output]
	};

	Ok(Tx::new(inputs, outputs))
    }

    pub fn sum(&self) -> u64 {
	let mut total = 0;
	for output in self.outputs.iter() {
	    total += output.value;
	}
	total
    }

    pub fn hash(&self) -> [u8; 32] {
	let encoded = bincode::serialize(self).unwrap();
	blake3::hash(&encoded).as_bytes().clone()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use rand::{CryptoRng, rngs::OsRng};
    use ed25519_dalek::Keypair;

    fn hash_public(keypair: &Keypair) -> [u8; 32] {
	let enc = bincode::serialize(&keypair.public).unwrap();
	blake3::hash(&enc).as_bytes().clone()
    }

    fn generate_coinbase(keypair: &Keypair, amount: u64) -> Tx {
	let pkh = hash_public(keypair);
	Tx::coinbase(pkh, amount)
    }

    #[actix_rt::test]
    async fn test_spend() {
	let mut csprng = OsRng{};
	let kp1 = Keypair::generate(&mut csprng);
	let kp2 = Keypair::generate(&mut csprng);

	let pkh1 = hash_public(&kp1);
	let pkh2 = hash_public(&kp2);

	// Generate a coinbase transaction and spend it
	let tx1 = generate_coinbase(&kp1, 1000);
	let tx2 = tx1.spend(&kp1, pkh2, pkh1, 900).unwrap();

	// Spending 0 is illegal
	let err1 = tx1.spend(&kp1, pkh2, pkh1, 0);
	assert_eq!(err1, Err(Error::ZeroSpend));
	// Spending the total should exceed available funds, since the fee is 100
	let err2 = tx1.spend(&kp1, pkh2, pkh1, 1000);
	assert_eq!(err2, Err(Error::ExceedsAvailableFunds));
	// Coinbase has 1 input thus one output is spent
	assert_eq!(tx2.inputs.len(), 1);
	// The sum of the outputs should be 1000 - fee = 900
	assert_eq!(tx2.sum(), 900 - fee);

	// Spend the result of spending the coinbase
	let tx3 = tx2.spend(&kp2, pkh1, pkh2, 700).unwrap();
	println!("{:?}", tx3.clone());
	assert_eq!(tx3.inputs.len(), 1);
	assert_eq!(tx3.sum(), 700 - fee);

	let err3 = tx1.spend(&kp1, pkh2, pkh1, 700);
	assert_eq!(err2, Err(Error::ExceedsAvailableFunds));
    }
}

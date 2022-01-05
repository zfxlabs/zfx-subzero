use super::{Result, Error};

use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signature, Signer};

pub type TxHash = [u8; 32];
pub type PublicKeyHash = [u8; 32];
pub type Amount = u64;

pub const fee: u64 = 100;

pub type OutputId = [u8; 32];

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Input {
    /// The hash of the source transaction.
    pub source: TxHash,
    /// The index of the output in the referenced transaction.
    pub i: u8,
    /// The public key of the owner.
    pub owner: PublicKey,
    /// The signature of the owner matching an output.
    pub signature: Signature,
}

impl Input {
    pub fn output_id(&self) -> OutputId {
	let bytes = vec![
	    self.source.clone().to_vec(),
	    vec![self.i.clone()],
	].concat();
	let encoded = bincode::serialize(&bytes).unwrap();
	blake3::hash(&encoded).as_bytes().clone()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Output {
    /// The public key hash of the owner.
    pub owner_hash: PublicKeyHash,
    /// The amount of tokens in the output.
    pub value: Amount,
}

impl Output {
    pub fn new(owner_hash: PublicKeyHash, value: Amount) -> Output {
	Output { owner_hash, value }
    }

    pub fn hash(&self) -> [u8; 32] {
	let encoded = bincode::serialize(self).unwrap();
	blake3::hash(&encoded).as_bytes().clone()
    }
}

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

    pub fn spend(&self, keypair: Keypair, destination: PublicKeyHash, change: PublicKeyHash, value: Amount) -> Result<Tx> {
	let owner = keypair.public.clone();

	// sum output amounts
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

	// consume outputs and construct inputs
	let mut i = 0;
	let mut running_total = 0;
	let mut amount_left = value;
	let mut inputs = vec![];
	let mut outputs = vec![];
	for output in self.outputs.iter() {
	    let output_hash = output.hash();
	    let signature = keypair.sign(&output_hash);
	    let input = Input {
		source: tx_hash.clone(),
		i: i.clone(),
		owner: owner.clone(),
		signature,
	    };
	    if amount_left > output.value + fee {
		let out = Output::new(destination.clone(), output.value);
		outputs.push(out);
		running_total += output.value;
		amount_left -= output.value;
	    } else {
		let final_amount = output.value - (amount_left - fee);
		let final_output = Output::new(destination.clone(), final_amount);
		outputs.push(final_output);
		if (output.value - fee) - final_amount > 0 {
		    let change_amount = (output.value) - fee - final_amount;
		    let change_output = Output::new(change.clone(), change_amount);
		    outputs.push(change_output);
		}
		break;
	    }
	    i += 1;
	}

	Ok(Tx::new(inputs, outputs))
    }

    pub fn hash(&self) -> [u8; 32] {
	let encoded = bincode::serialize(self).unwrap();
	blake3::hash(&encoded).as_bytes().clone()
    }
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

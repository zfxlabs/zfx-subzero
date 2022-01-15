use super::input::Input;
use super::inputs::Inputs;
use super::output::{Amount, Output, PublicKeyHash};
use super::outputs::Outputs;
use super::{Error, Result};

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use ed25519_dalek::Keypair;

pub type TxHash = [u8; 32];

pub const FEE: u64 = 100;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Tx {
    pub inputs: Inputs<Input>,
    pub outputs: Outputs<Output>,
}

impl Hash for Tx {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inputs.hash(state);
        self.outputs.hash(state);
    }
}

impl Ord for Tx {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.inputs.cmp(&other.inputs) {
            Ordering::Equal => self.outputs.cmp(&other.outputs),
            ord => ord,
        }
    }
}

impl PartialOrd for Tx {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.inputs.cmp(&other.inputs) {
            Ordering::Equal => Some(self.outputs.cmp(&other.outputs)),
            ord => Some(ord),
        }
    }
}

impl Tx {
    pub fn new(inputs: Vec<Input>, outputs: Vec<Output>) -> Tx {
        Tx { inputs: Inputs::new(inputs), outputs: Outputs::new(outputs) }
    }

    pub fn inputs(&self) -> Vec<Input> {
        let mut v: Vec<Input> = self.inputs.iter().cloned().collect();
        v.sort();
        v
    }

    pub fn outputs(&self) -> Vec<Output> {
        self.outputs.outputs.clone()
    }

    pub fn coinbase(owner: PublicKeyHash, value: Amount) -> Tx {
        Tx::new(vec![], vec![Output::new(owner, value)])
    }

    pub fn spend(
        &self,
        keypair: &Keypair,
        tx_hash: TxHash,
        destination: PublicKeyHash,
        change: PublicKeyHash,
        amount: Amount,
    ) -> Result<Tx> {
        self.validate_amount(amount, false)?;

        // Consume outputs and construct inputs, remaining inputs should be reflected in
        // the change amount.
        let mut i = 0;
        let mut amount_to_spend = amount.clone();
        let mut change_amount = 0;
        let mut consumed = 0;
        let mut inputs = vec![];

        for output in self.outputs().iter() {
            if consumed < amount {
                inputs.push(Input::new(keypair, tx_hash.clone(), i.clone()));

                if amount_to_spend >= output.value {
                    amount_to_spend -= output.value;
                    consumed += output.value;
                } else {
                    consumed += amount_to_spend;
                    change_amount = output.value - amount_to_spend;
                }
                i += 1;
            } else {
                break;
            }
        }

        // Aggregate the spent value into one main output.
        let main_output = Output::new(destination.clone(), consumed.clone());
        // Create a change output.
        let outputs = if change_amount > FEE && change_amount - FEE > 0 {
            vec![main_output, Output::new(change.clone(), change_amount.clone() - FEE)]
        } else {
            vec![main_output]
        };

        Ok(Tx::new(inputs, outputs))
    }

    pub fn stake(&self, keypair: &Keypair, change: PublicKeyHash, amount: Amount) -> Result<Tx> {
        self.validate_amount(amount, true)?;

        let tx_hash = &self.hash();

        // Consume outputs and construct inputs, remaining inputs should be reflected in
        // the change amount.
        let mut amount_to_stake = amount.clone() + FEE;
        let mut change_amount = 0;
        let mut inputs = vec![];
        let mut i = 0;
        for output in self.outputs().iter() {
            let input = Input::new(keypair, tx_hash.clone(), i.clone());
            inputs.push(input);
            if amount_to_stake > output.value {
                amount_to_stake -= output.value;
            } else if amount_to_stake == output.value {
                amount_to_stake = 0;
            } else {
                // if amount_to_stake < output.value
                let value = output.value - amount_to_stake;
                change_amount += value;
            }
            i += 1;
        }

        // Create a change output.
        let outputs = if change_amount > 0 {
            vec![Output::new(change.clone(), change_amount.clone())]
        } else {
            vec![]
        };

        Ok(Tx::new(inputs, outputs))
    }

    pub fn sum(&self) -> u64 {
        let mut total = 0;
        for output in self.outputs().iter() {
            total += output.value;
        }
        total
    }

    pub fn hash(&self) -> [u8; 32] {
        let encoded = bincode::serialize(self).unwrap();
        blake3::hash(&encoded).as_bytes().clone()
    }

    fn validate_amount(&self, amount: Amount, is_stake_amount: bool) -> Result<()> {
        let mut total = self.sum();
        if amount == 0 {
            return if is_stake_amount == true {
                Err(Error::ZeroStake)
            } else {
                Err(Error::ZeroSpend)
            };
        }
        if amount > total - FEE {
            return Err(Error::ExceedsAvailableFunds);
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use ed25519_dalek::Keypair;

    #[actix_rt::test]
    async fn test_spend_with_three_outputs() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        let tx1 = Tx::new(
            vec![],
            vec![
                Output::new(pkh1.clone(), 700),
                Output::new(pkh1.clone(), 1000),
                Output::new(pkh1.clone(), 300),
            ],
        );
        let tx2 = tx1.spend(&kp1, pkh2, pkh1, tx1.hash(), 1800).unwrap();

        assert_eq!(tx2.inputs.len(), 3);
        assert_eq!(tx2.outputs()[0].value, 1800); // total spent - fee
        assert_eq!(tx2.outputs()[1].value, 100); // remaining spendable amount
    }

    #[actix_rt::test]
    async fn test_spend_zero_then_throw_error() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        let tx1 = generate_coinbase(&kp1, 1000);
        assert_eq!(tx1.spend(&kp1, pkh2, pkh1, tx1.hash(), 0), Err(Error::ZeroSpend));
    }

    #[actix_rt::test]
    async fn test_spend_more_than_allowed_then_throw_error() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        let tx = generate_coinbase(&kp1, 1000);

        assert_eq!(tx.spend(&kp1, pkh2, pkh1, tx.hash(), 1000), Err(Error::ExceedsAvailableFunds));
        assert_eq!(
            tx.spend(&kp1, pkh2, pkh1, tx.hash(), 1001 - FEE),
            Err(Error::ExceedsAvailableFunds)
        ); // including fee
    }

    #[actix_rt::test]
    async fn test_spend_various_amounts() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        // Generate a coinbase transaction and spend it
        let tx1 = generate_coinbase(&kp1, 1000);
        let tx2 = tx1.spend(&kp1, pkh2, pkh1, tx1.hash(), 900).unwrap();

        assert_eq!(tx2.inputs.len(), 1);
        assert_eq!(tx2.outputs.len(), 1);
        // The sum of the outputs should be 1000 - fee = 900
        assert_eq!(tx2.sum(), 900);

        // Spend the result of spending the coinbase
        let tx3 = tx2.spend(&kp2, pkh1, pkh2, tx2.hash(), 700).unwrap();
        assert_eq!(tx3.inputs.len(), 1);
        // The sum should take into account the change amount
        assert_eq!(tx3.sum(), 800);

        let tx4 = tx3.spend(&kp1, pkh2, pkh1, tx3.hash(), 700).unwrap();
        assert_eq!(tx4.sum(), 700);
        assert_eq!(tx4.outputs.len(), 1);
    }

    #[actix_rt::test]
    async fn test_stake_zero_then_throw_error() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        assert_eq!(generate_coinbase(&kp1, 1000).stake(&kp1, pkh2, 0), Err(Error::ZeroStake));
    }

    #[actix_rt::test]
    async fn test_stake_more_than_allowed_then_throw_error() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        let tx = generate_coinbase(&kp1, 1000);

        assert_eq!(tx.stake(&kp1, pkh2, 1000), Err(Error::ExceedsAvailableFunds));
        assert_eq!(tx.stake(&kp1, pkh2, 1001 - FEE), Err(Error::ExceedsAvailableFunds));
    }

    #[actix_rt::test]
    async fn test_stake() {
        let (kp1, kp2, pkh1, pkh2) = generate_keys();

        // Generate a coinbase transaction and stake it
        let tx1 = generate_coinbase(&kp1, 1000);
        let tx2 = tx1.stake(&kp1, pkh2, 900).unwrap();

        assert_eq!(tx2.inputs.len(), 1);
        assert_eq!(tx2.outputs.len(), 0);
        // The sum of the outputs should be 0
        assert_eq!(tx2.sum(), 0);

        // Stake half the amount in a coinbase tx
        let tx3 = tx1.stake(&kp2, pkh1, 500).unwrap();
        assert_eq!(tx3.inputs.len(), 1);
        assert_eq!(tx3.outputs.len(), 1);
        assert_eq!(tx3.sum(), 500 - FEE);
    }

    fn hash_public(keypair: &Keypair) -> [u8; 32] {
        let enc = bincode::serialize(&keypair.public).unwrap();
        blake3::hash(&enc).as_bytes().clone()
    }

    fn generate_coinbase(keypair: &Keypair, amount: u64) -> Tx {
        let pkh = hash_public(keypair);
        Tx::coinbase(pkh, amount)
    }

    fn generate_keys() -> (Keypair, Keypair, [u8; 32], [u8; 32]) {
        let kp1_hex = "ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416".to_owned();
        let kp2_hex = "5a353c630d3faf8e2d333a0983c1c71d5e9b6aed8f4959578fbeb3d3f3172886393b576de0ac1fe86a4dd416cf032543ac1bd066eb82585f779f6ce21237c0cd".to_owned();

        let kp1 = Keypair::from_bytes(&hex::decode(kp1_hex).unwrap()).unwrap();
        let kp2 = Keypair::from_bytes(&hex::decode(kp2_hex).unwrap()).unwrap();

        let pkh1 = hash_public(&kp1);
        let pkh2 = hash_public(&kp2);

        return (kp1, kp2, pkh1, pkh2);
    }
}

use super::TxHash;

use std::cmp::{Ord, Ordering};
use std::hash::{Hash, Hasher};

use ed25519_dalek::{Keypair, PublicKey, Signature, Signer};

pub type UTXOId = [u8; 32];

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
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

impl std::fmt::Debug for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let source = format!("{}", hex::encode(self.source));
        write!(f, "{{source={},i={:?}}}", source, self.i)
    }
}

// FIXME: Error prone serialization / comparisons

impl Ord for Input {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.source.cmp(&other.source) {
            Ordering::Equal => match self.i.cmp(&other.i) {
                Ordering::Equal => {
                    let self_owner = bincode::serialize(&self.owner).unwrap();
                    let other_owner = bincode::serialize(&other.owner).unwrap();
                    match self_owner.cmp(&other_owner) {
                        Ordering::Equal => {
                            let self_signature = bincode::serialize(&self.signature).unwrap();
                            let other_signature = bincode::serialize(&other.signature).unwrap();
                            self_signature.cmp(&other_signature)
                        }
                        ord => ord,
                    }
                }
                ord => ord,
            },
            ord => ord,
        }
    }
}

impl PartialOrd for Input {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.source.cmp(&other.source) {
            Ordering::Equal => match self.i.cmp(&other.i) {
                Ordering::Equal => {
                    let self_owner = bincode::serialize(&self.owner).unwrap();
                    let other_owner = bincode::serialize(&other.owner).unwrap();
                    match self_owner.cmp(&other_owner) {
                        Ordering::Equal => {
                            let self_signature = bincode::serialize(&self.signature).unwrap();
                            let other_signature = bincode::serialize(&other.signature).unwrap();
                            Some(self_signature.cmp(&other_signature))
                        }
                        ord => Some(ord),
                    }
                }
                ord => Some(ord),
            },
            ord => Some(ord),
        }
    }
}

impl Hash for Input {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.hash(state);
        self.i.hash(state);
        let owner = bincode::serialize(&self.owner).unwrap();
        owner.hash(state);
        let signature = bincode::serialize(&self.signature).unwrap();
        signature.hash(state);
    }
}

impl Input {
    pub fn new(keypair: &Keypair, source: TxHash, i: u8) -> Self {
        let bytes = vec![source.clone().to_vec(), vec![i.clone()]].concat();
        let encoded = bincode::serialize(&bytes).unwrap();
        let hash = blake3::hash(&encoded).as_bytes().clone();
        let signature = keypair.sign(&hash);
        Input { source, i, owner: keypair.public.clone(), signature }
    }

    pub fn utxo_id(&self) -> UTXOId {
        let bytes = vec![self.source.clone().to_vec(), vec![self.i.clone()]].concat();
        let encoded = bincode::serialize(&bytes).unwrap();
        blake3::hash(&encoded).as_bytes().clone()
    }
}

use super::TxHash;

use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signature, Signer};

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

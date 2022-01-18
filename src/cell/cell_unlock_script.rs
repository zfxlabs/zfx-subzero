use std::cmp::{Ord, Ordering};
use std::hash::{Hash, Hasher};

use ed25519_dalek::{PublicKey, Signature};

/// A cells unlocking script (simple).
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct CellUnlockScript {
    public_key: PublicKey,
    signature: Signature,
}

impl CellUnlockScript {
    pub fn new(public_key: PublicKey, signature: Signature) -> Self {
        CellUnlockScript { public_key, signature }
    }
}

impl Ord for CellUnlockScript {
    // FIXME
    fn cmp(&self, other: &Self) -> Ordering {
        let self_pks = bincode::serialize(&self.public_key).unwrap();
        let other_pks = bincode::serialize(&other.public_key).unwrap();
        match self_pks.cmp(&other_pks) {
            Ordering::Equal => {
                let self_sig = bincode::serialize(&self.signature).unwrap();
                let other_sig = bincode::serialize(&other.signature).unwrap();
                self_sig.cmp(&other_sig)
            }
            ord => ord,
        }
    }
}

impl PartialOrd for CellUnlockScript {
    // FIXME
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let self_pks = bincode::serialize(&self.public_key).unwrap();
        let other_pks = bincode::serialize(&other.public_key).unwrap();
        match self_pks.cmp(&other_pks) {
            Ordering::Equal => {
                let self_sig = bincode::serialize(&self.signature).unwrap();
                let other_sig = bincode::serialize(&other.signature).unwrap();
                Some(self_sig.cmp(&other_sig))
            }
            ord => Some(ord),
        }
    }
}

impl Hash for CellUnlockScript {
    // FIXME
    fn hash<H: Hasher>(&self, state: &mut H) {
        let pks = bincode::serialize(&self.public_key).unwrap();
        let sig = bincode::serialize(&self.signature).unwrap();
        pks.hash(state);
        sig.hash(state);
    }
}

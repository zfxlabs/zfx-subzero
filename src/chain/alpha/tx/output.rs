use std::cmp::{Ord, Ordering};
use std::hash::{Hash, Hasher};

pub type Amount = u64;
pub type PublicKeyHash = [u8; 32];

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Output {
    /// The public key hash of the owner.
    pub owner_hash: PublicKeyHash,
    /// The amount of tokens in the output.
    pub value: Amount,
}

impl std::fmt::Debug for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let owner = format!("{}", hex::encode(self.owner_hash));
        write!(f, "{{owner={},value={:?}}}", owner, self.value)
    }
}

// FIXME: Error prone serialization / comparisons

impl Ord for Output {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.owner_hash.cmp(&other.owner_hash) {
            Ordering::Equal => self.value.cmp(&other.value),
            ord => ord,
        }
    }
}

impl PartialOrd for Output {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.owner_hash.cmp(&other.owner_hash) {
            Ordering::Equal => Some(self.value.cmp(&other.value)),
            ord => Some(ord),
        }
    }
}

impl Hash for Output {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.owner_hash.hash(state);
        self.value.hash(state);
    }
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

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

impl Output {
    pub fn new(owner_hash: PublicKeyHash, value: Amount) -> Output {
        Output { owner_hash, value }
    }

    pub fn hash(&self) -> [u8; 32] {
        let encoded = bincode::serialize(self).unwrap();
        blake3::hash(&encoded).as_bytes().clone()
    }
}

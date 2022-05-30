use std::convert::TryInto;
use std::fmt;
use std::net::SocketAddr;
use std::ops::Index;
use std::str::FromStr;

use base58check::{FromBase58Check, ToBase58Check};
use blake2::digest::{Update, VariableOutput};
use blake2::Blake2bVar;
use rand::{self, Rng};

/// Generic hash-based ID for use throughout the system
#[derive(Hash, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Serialize, Deserialize, Default)]
pub struct Id(pub [u8; 32]);

impl std::fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.to_base58check(0))
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.to_base58check(0))
    }
}

impl FromStr for Id {
    type Err = crate::Error;

    /// Converts a base58check encoded string to bytes of an Id
    fn from_str(id_str: &str) -> Result<Self, crate::Error> {
        let (vsn, bytes) =
            id_str.from_base58check().map_err(|_| crate::Error::TryFromStringError)?;
        if vsn != 0 {
            return Err(crate::Error::TryFromStringError);
        }
        let bytes: [u8; 32] =
            bytes.as_slice().try_into().map_err(|_| crate::Error::TryFromStringError)?;
        Ok(Id(bytes))
    }
}

impl Id {
    /// By default a new id is created by hashing an input byte slice
    pub fn new(bytes: &[u8]) -> Id {
        Id(hash(bytes))
    }

    /// Sets the bytes of an Id explicitly (expects a hash)
    pub fn from_hash(bytes: &[u8]) -> Id {
        let mut byte_vec = bytes.to_vec();
        byte_vec.resize(32, 0u8);
        let boxed_slice = byte_vec.into_boxed_slice();
        let boxed_array: Box<[u8; 32]> = boxed_slice.try_into().unwrap();
        Id(*boxed_array)
    }

    /// Converts a `SocketAddr` into an *untrusted* identity.
    pub fn from_ip(ip: &SocketAddr) -> Id {
        Id::new(format!("{:?}", ip.clone()).as_bytes())
    }

    pub fn generate() -> Id {
        let mut rng = rand::thread_rng();
        let v: [u8; 32] = rng.gen();
        Id(v)
    }

    pub fn zero() -> Id {
        Id([0u8; 32])
    }

    pub fn max() -> Id {
        Id([255u8; 32])
    }

    pub fn one() -> Id {
        Id([1u8; 32])
    }
    pub fn two() -> Id {
        Id([2u8; 32])
    }
    pub fn test_one() -> Id {
        Id([
            0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
            0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0x80u8,
        ])
    }

    pub fn bytes(&self) -> [u8; 32] {
        self.0
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Hashes (256-bit) a u64 and prepends it to a key in order generate a new one.
    pub fn hash_prefix(&self, prefix: u64) -> Id {
        let mut bytes: Vec<u8> = prefix.to_be_bytes().to_vec();
        let mut id_bytes: Vec<u8> = self.0.to_vec();
        bytes.append(&mut id_bytes);
        Id(hash(&bytes))
    }

    /// Prefixes an Id with the hash of a u64.
    pub fn prefix(&self, prefix: u64) -> [u8; 64] {
        let prefix: [u8; 32] = hash(&prefix.to_be_bytes());
        let id_bytes: [u8; 32] = self.0;
        let mut prefixed = [0u8; 64];
        prefixed[..32].clone_from_slice(&prefix[..32]);
        prefixed[32..64].clone_from_slice(&id_bytes[..32]);
        prefixed
    }

    /// Prefixes an Id with another id.
    pub fn prefix_id(&self, prefix: Id) -> [u8; 64] {
        let id_bytes: [u8; 32] = self.bytes();
        let prefix: [u8; 32] = prefix.bytes();
        let mut prefixed = [0u8; 64];
        prefixed[..32].clone_from_slice(&prefix[..32]);
        prefixed[32..64].clone_from_slice(&id_bytes[..32]);
        prefixed
    }

    /// Suffixes an Id with an 8-byte array (e.g a TAI64 time).
    pub fn suffix(&self, suffix: [u8; 8]) -> [u8; 40] {
        let id_bytes: [u8; 32] = self.bytes();
        let mut suffixed = [0u8; 40];
        suffixed[..32].clone_from_slice(&id_bytes[..32]);
        suffixed[32..40].clone_from_slice(&suffix[..8]);
        suffixed
    }
}

// overloads array indexing (e.g: id[1] = first byte of id)
impl Index<usize> for Id {
    type Output = u8;

    fn index(&self, i: usize) -> &u8 {
        &self.0[i]
    }
}

// overloads array range indexing (e.g: id[1..3])
impl Index<std::ops::Range<usize>> for Id {
    type Output = [u8];

    fn index(&self, r: std::ops::Range<usize>) -> &[u8] {
        &self.0[r]
    }
}

// This function is the replacement for `zfx_crypto`s `hash!` macro
pub fn hash(input: &[u8]) -> [u8; 32] {
    let mut hasher = Blake2bVar::new(32).unwrap();
    hasher.update(input);
    let mut buf = [0u8; 32];
    hasher.finalize_variable(&mut buf).unwrap();
    buf
}

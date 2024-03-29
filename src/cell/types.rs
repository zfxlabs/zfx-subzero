/// Default fee for making a transaction (ex. transfer or staking balance)
pub const FEE: u64 = 3;

/// The capacity of a particular cell (size in bytes).
pub type Capacity = u64;

/// The public key hash of some signer.
pub type PublicKeyHash = [u8; 32];

/// The hash of a cell.
pub type CellHash = [u8; 32];

/// Represents a type of cell, depending on applied operation on it
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum CellType {
    /// This type is assigned to [Output] to represent a balance
    /// held for an account. This balance can be [transferred][TransferOperation]
    /// to another account or [staked][StakeOperation] for an account.
    /// [CoinbaseOperation] creates [Output] with this type.
    Coinbase,
    /// This type is assigned to [Output] to represent a transfer balance
    /// to account, for example when transferring balance from one account to another.
    /// [TransferOperation] creates [Output] with this type.
    Transfer,
    /// This type is assigned to [Output] to represent an initial stake balance
    /// for an account which can expire over time and can be used to stake the network,
    /// for example when form a genesis block.
    /// [StakeOperation] creates [Output] with this type.
    Stake,
}

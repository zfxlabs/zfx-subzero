/// Represents a type of cell, depending on applied operation on it
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum CellType {
    /// This type is assigned to [Output][crate::cell::output::Output] to represent a balance
    /// held for an account. This balance can be [transferred][crate::alpha::transfer::TransferOperation]
    /// to another account or [staked][crate::alpha::stake::StakeOperation] for an account.
    /// [CoinbaseOperation][crate::alpha::coinbase::CoinbaseOperation] creates [Output][crate::cell::output::Output] with this type.
    Coinbase,
    /// This type is assigned to [Output][crate::cell::output::Output] to represent a transfer balance
    /// to account, for example when transferring balance from one account to another.
    /// [TransferOperation][crate::alpha::transfer::TransferOperation] creates [Output][crate::cell::output::Output] with this type.
    Transfer,
    /// This type is assigned to [Output][crate::cell::output::Output] to represent an initial stake balance
    /// for an account which can expire over time and can be used to stake the network,
    /// for example when form a genesis block.
    /// [StakeOperation][crate::alpha::stake::StakeOperation] creates [Output][crate::cell::output::Output] with this type.
    Stake,
}

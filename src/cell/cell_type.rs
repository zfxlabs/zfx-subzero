#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum CellType {
    Coinbase,
    Transfer,
    Stake,
}

/// Compute the `hail` consensus weight based on the number of tokens a validator has.
pub fn percent_of(qty: u64, total: u64) -> f64 {
    qty as f64 / total as f64
}

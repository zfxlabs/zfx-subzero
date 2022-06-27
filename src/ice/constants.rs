use tokio::time::Duration;

// Network settings

// TODO: currently not used
// pub const ROUND_TRIP_TIME: Duration = Duration::from_secs(3);

/// One protocol round (6 seconds)
pub const PROTOCOL_PERIOD: Duration = Duration::from_secs(6);

// Gossip settings
// TODO: currently unused
// /// Number of gossip messages in a `Ping`
// pub const GOSSIP_RATE: usize = 3;

/// Maximum number of peers pinged in one protocol round
pub const PING_MAX_SIZE: usize = 11;

// Consensus settings

/// Alpha parameter (percent convergence required for a vote)
pub const ALPHA: f64 = 0.5;
/// Kappa parameter (fanout)
pub const K: usize = 2;
/// Beta one parameter (safe precommit)
pub const BETA1: usize = 3;

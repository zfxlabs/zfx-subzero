use tokio::time::Duration;

// Network settings
pub const ROUND_TRIP_TIME: Duration = Duration::from_secs(3);
pub const PROTOCOL_PERIOD: Duration = Duration::from_secs(6);

// Gossip settings
pub const GOSSIP_RATE: usize = 3;

// Consensus settings

// Sleet alpha parameter (percent convergence required for a vote)
pub const ALPHA: f64 = 0.5;
// Sleet kappa parameter (fanout)
pub const K: usize = 2;
// Sleet beta one parameter (safe precommit)
pub const BETA1: usize = 3;

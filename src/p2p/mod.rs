//! Definition of peer to peer primitives.
pub mod connection;
pub mod connection_factory;
pub mod connection_handler;
pub mod id;
pub mod peer_meta;
pub mod prelude;
pub mod response_handler;
pub mod sender;

/// The `LinearBackoff` sends `Execute` messages periodically to a `peer_bootstrapper`
pub mod linear_backoff;
/// The `PeerBootstrapper` sends `ReceivePeerSet` to the primary network bootstrapper.
pub mod peer_bootstrapper;
/// The `PrimaryBootstrapper` aggregates a group of bootstrap validators which are subscribed to the
/// primary network and forwards them to the `PrimarySynchroniser`, responsible for obtaining as much
/// of the cell state from the trusted peers as is currently available at those peers.
pub mod primary_bootstrapper;

// network bootstrapper sorts peers by chain and sends accumulated bootstrap peers via the
// a `chain_bootstrapper`.
//pub mod network_bootstrapper;
// chain bootstrapper sends `ReceiveBootstrapQuorum` to `ice` (primary)
//pub mod chain_bootstrapper;

// `ice` sends `LivePeers` to consensus
// consensus:
//   * requests transactions from `sleet` to build and propose blocks
//   * receives new blocks from `ice` gossip

pub mod connection;
pub mod connection_factory;
pub mod connection_handler;
pub mod peer_meta;
pub mod prelude;
pub mod response_handler;
pub mod sender;

// linear backoff sends `Execute` to peer bootstrapper
pub mod linear_backoff;
// peer bootstrapper sends `ReceivePeerGroup` to the network bootstrapper.
pub mod peer_bootstrapper;
// network bootstrapper sorts peers by chain and sends accumulated bootstrap peers via the
// a `chain_bootstrapper`.
pub mod network_bootstrapper;
// chain bootstrapper sends `ReceiveBootstrapQuorum` to `ice`
pub mod chain_bootstrapper;

// `ice` sends `LivePeers` to consensus
// consensus:
//   * requests transactions from `sleet` to build and propose blocks
//   * receives new blocks from `ice` gossip

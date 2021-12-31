# ice

`ice` defines a reservoir sampling based consensus mechanism and a gossip protocol for performing a safe (trusted / weightless) bootstrap of the `alpha` chain. The weightless aspect of the algorithm means that it is not resistant to sybil attacks and therefore requires a trusted `whitelist` initially.

`ice` is augmented with sybil resistance once the `alpha` chain is bootstrapped, which defines the validator set.

# Purpose

The purpose of `ice` is to ensure that a sufficient number of peers are `Live` in the network prior to bootstrapping the `alpha` chain and to become sybil resistant once `alpha` is bootstrapped. Thereafter `ice` disseminates and handles gossip to peers.

# How does `ice` work?

`ice` performs consensus over a hash table of elements by using reservoir sampling.

The reservoir stores binary consensus instances for every bootstrap peer. Every round a peer is sampled uniformly at random from the network `view` and a `Ping` request is sent containing a vector of up to `k` queries such that every query pertains to an independent peer to be queried by the receiving node.

A node receiving a vector of queries from another peer must respond a vector of `Outcome`s, corresponding to the queries received, containing whether the node currently believes the peer to be `Live` or `Faulty`. 

The outcomes received are used to fill the reservoir with choices contained therein which are subsequently used to influence a decision concerning the designated peers liveness status. A quorum within the reservoir is said to be `full` if `k` entries have been recorded about a particular peer (note that every round at most 1 entry per peer is permitted and thus there must be at minimum `k` rounds to fill one sample).

When a sample becomes `full` in the reservoir (when `k` choices have been recorded for a particular peer) the quorum becomes `decidable`, such that an `Outcome` may be produced designating the status of the peer according to consensus.

In `ice` each decision also has a notion of `conviction`, which is used to add a notion of safety to a peers liveness. We wish for decisions made about a peers liveness made under consensus to be relatively coherent - note that in `ice` the only safety parameter is `beta1` and there is no `beta2` for achieving strong finality, since the nature of connecting and disconnecting is transient and prone to error.

For those who have read the `Snow*` papers, `ice` is a specialisation of the `Snowball` consensus algorithm to a hash table of binary consensus instance. This is a parallel to `Avalanche` which specialises `Snowball` to directed acyclic graphs.

The pinging protocol is a modification of `SWIM` which replaces indirect pings with pings over the state of a particular peer.

# Analysis

Since the protocol uses only 1 ping per round containing a message of size `k`, the message complexity of the algorithm is `O(1)` and resolves in `n/k * q * beta1 - decided(n)` rounds, where `n` is the number of peers, `k` is the amount of queries per `Ping`, `q` is the amount of outcomes required to reach a quorum, `beta1` is the safety threshold (the amount of quorums required for finality) and `decided(n)` is the number of peers whose status has reached a decided state. The worst case number of rounds in order to assess whether every peer is live or `Faulty` is thus `n/k * q * beta1`, where none of the peers on the network have an initial decided state.

For example for a preliminary network of size `5`, where each `Ping` contains `5` queries, where the amount of consecutive outcomes required to reach a decision is `3` and `beta1` is `3` requires at minimum 9 rounds for all the honest peers to become final.

The algorithm would take longer to resolve under a byzantine scenario where adversarial nodes are allowed to influence the outcome (e.g. outside of a safe bootstrap). In this particular case it is not an issue since in order to bootstrap `alpha`, only `k` nodes are required to be honest, whilst the dishonest nodes are culled when `ice` (the sybil resistant consensus algorithm) is bootstrapped. However a `whitelist` / list of bootstrap seeds are used in order to thwart attacks which could slow down the bootstrapping process.

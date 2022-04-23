# zfx-subzero Integration Proposal #1
## Integration to Tezos

### Analysis
The milestone #4 deliverables are modified from the original SoW in order to prioritise bridging between a network with another. The current `deku` solution uses a Tendermint light-client which communicates with a network running Tendermint consensus in order to obtain fraud proofs when an invalid state transition occurs.

### Proposed solution
We propose providing a custom light-client which communicates with a network running `hail` consensus instead. The solutions are similar in the sense that a network of validator nodes are expected to provide fraud proofs to light clients interactively but the `hail` solution does not require the use of signatures on blocks, since consensus in `hail` operates on signatures at the network level.

Proposed deliverables:

* Extend blocks with merkle proofs for proving inclusion.
* Extend `hail` (block based consensus mechanism) with fraud proofs for proving an invalid state transition.
* Produce a `hail` light-client which consumes block headers and receives fraud proofs when an invalid state transition occurs, such that the light client can discard invalid blocks.

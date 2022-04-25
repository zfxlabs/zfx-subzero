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

## Block structure extension

The block structure is extended with the following fields:
```
/// The root of the merkle tree of the data (e.g. cells) included in the block.
data_root
/// The number of leaves represented by the data_root.
data_length
/// The root of a sparse merkle tree of the current state of the chain.
state_root
```

## State Root

The state root in our model is represented by a key-value map where the keys are transaction output identifiers (e.g. `hash(hash(d)||i)` where `d` is the data of the transaction and `i` is the index of the output being referred to in `d`. The value of each key is the state of each transaction output: either `unspent(1)` or `nonexistent(0)` - the default value.

In addition to a regular transition function, we define a `root_transition` function which performs transitions without requiring the whole state tree, but only the state root and merkle proofs of parts of the state tree which the transaction reads or modifies.

These merkle proofs are expressed as a sub-tree of the same tree with a common root:
```
root_transition(state_root, t, w) E {state_root, err}
```

A state witness `w` consists of a set of key-value pairs and their associated sparse merkle proofs in the state tree, `w = {(k1, v1, {k1, v1 -> state_root}), ..}`.

## Fraud Proofs

A faulty or malicious miner may provide an incorrect `state_root_i`. We use the execution trace provided in `data_root_i` to prove that a part of the execution trace was invalid.

We defined a function `verify_transition_fraud_proof` and its parameters which verify fraud proofs received from full nodes.

A fraud proof consists of the relevant shares in the block which contain a bad state transition, merkle proofs for those shares and the state witnesses for the transactions contained within those shares. The function takes as input a fraud proof and checks if applying the transactions in a period of the blocks data on the intermediate pre-state root results in the intermediate post-state root specified in the block data. It it does not then the fraud proof is valid and the block that the fraud proof is for should be permanently rejected by the light client.

## Data Availability Proofs

A malicious block producer can prevent full nodes from generating fraud proofs by withholding the data needed to compute the `data_root_i` and only release the block header to the network.

We employ a data availability scheme based on reed-solomon erasure coding, where light clients request random shares of data to get high probability guarantees that the data associated with the root of a merkle tree is available. The scheme assumes there is a sufficient number of honest light clients making the same requests such that the network can recover the data as light clients upload the shares to full nodes and if a full node does not have the complete data requests it.

Note: It is fundamental for light clients to have assurance that all the transaction data is available since it is only necessary to withhold a few bytes to hide an invalid transaction in a block.

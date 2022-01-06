# zfx-subzero

The `zfx-subzero` project is a unification of the core products which `zero.fx` has been working on throughout the year into a unified whole.

# ice

A `O(1)` reservoir sampling based consensus algorithm for transiently establishing the liveness of peers and performing a safe network bootstrap.

Once the `alpha` chain is instantiated, `ice` becomes sybil resistant and is augmented with information about peers such as the stake `amount` and `uptime`.

Please see the `ice` subdirectory for further details.

# sleet

A consensus algorithm based on `Avalanche` but specialised to mempools. 

Please see the `sleet` subdirectory for further information.

# hail

A consensus algorithm with similar properties as `Avalanche` but specialised to blocks. 

Please see the `hail` subdirectory for more information.

# alpha

The root chain of the `zero.fx` network. 

# bridge

Work for `m4` surrounding bridging will go in this subdirectory, concerning bridging assets to `Tezos` and back.

## Unified Overview

How the components fit together:
1. Ice performs a safe bootstrap with trusted peers and establishes liveness based on reservoir sampling consensus.
2. Once `ice` obtains sufficient live peers, the `alpha` chain state is bootstrapped and used to add sybil resistance to `ice` based on the latest validator set. 
3. `sleet` consensus (mempool) is initialised with the latest validator set in order to query peers about transactions. The `alpha` frontier of final transactions is sent to `sleet` in order to provision the roots of new transactions.
4. Transactions are posted to `sleet` by the client in order to spend funds (e.g. sending from account A to B on the alpha chain). `sleet` resolves conflicts between these transactions, ensuring that only transactions which do not conflict (spend the same funds) eventually become final.
5. `hail` is initialised with the latest validator set in the same way as `sleet`. Whenever the VRF based selection selects the validator running `hail`, final transactions in `sleet` are used to generate a new block. `hail` resolves conflicts between blocks, ensuring that whenever a block conflicts at the same height the block with the lowest hash is selected.

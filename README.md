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

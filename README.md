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

## Running the local testnet

```
cargo run --bin node -- -a 127.0.0.1:1234 -b 127.0.0.1:1235 --keypair ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416

cargo run --bin node -- -a 127.0.0.1:1235 -b 127.0.0.1:1234 --keypair 5a353c630d3faf8e2d333a0983c1c71d5e9b6aed8f4959578fbeb3d3f3172886393b576de0ac1fe86a4dd416cf032543ac1bd066eb82585f779f6ce21237c0cd

cargo run --bin node -- -a 127.0.0.1:1236 -b 127.0.0.1:1235 --keypair 6f4b736b9a6894858a81696d9c96cbdacf3d49099d212213f5abce33da18716f067f8a2b9aeb602cd4163291ebbf39e0e024634f3be19bde4c490465d9095a6b
```

## Running the client test

```
cargo run --bin client_test -- --peer-ip 127.0.0.1:1234 --keypair ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416 --cell-hash b5fba12b605e166987f031c300e33969e07e295285a3744692f326535fba555e # --loop 16
```

# zfx-subzero
![Build](https://github.com/zfxlabs/zfx-subzero/actions/workflows/main.yml/badge.svg?branch=main)

The `zfx-subzero` project is a unification of the core products which `zero.fx` has been working on throughout the year.

For build and test instructions, see [below](#build-and-test).

The purpose of `subzero` is provide a network which can reach consensus on blocks containing operations for potentially multiple distinct blockchains. `subzero` acts as a consensus and storage layer, delegating the task of executing state transitions and verifying the specific contents of operations to other client chains.

The `alpha` primitives are the only exception to this rule. `alpha`s purpose is to define very simple primitives which allow for an economic model to exist (primitives for transfers and staking namely), so that there is a notion of state capacity on the network (this is necessary in order to provide sybil resistance).

The network is comprised of `cell` based transactions, which is an extension to the `utxo` model to include a `data` field and a cell type. This enables transactions to contain arbitrary data where the cell type defines the interpretation of the cell and is only relevant to clients which support that specific cell type.

The `alpha` primitives are the only types of `cell` which must be executed as part of consensus in order to determine whether validator staking times become invalid or when new validators begin staking on the network.

There are three layers of consensus in `subzero`, each of which provide a vital role enabling the subsequent consensus mechanisms to operate.

# ice

A `O(1)` reservoir sampling based consensus algorithm for transiently establishing the liveness of peers and performing a safe network bootstrap.

Once the `alpha` chain is instantiated, `ice` becomes sybil resistant and is augmented with information about peers such as the stake `amount` and `uptime`.

Please see the `ice` subdirectory for further details.

# sleet

`sleet` is a consensus algorithm based on `Avalanche` and the closest one to the original papers. The purpose of `sleet` is to resolve conflicts between `cell`-based transactions and ensure that a double spending transaction never becomes live, nor adopted in a subsequent block.

`sleet` ensures that cells do not conflict but do not execute state contained within. Thus if the `cell` can be deserialized, has the right form and spends from a valid `capacity`, it will be accepted in a block. It is then up to the receivers of the block to determine whether its inner contents (the `data` field) are valid.

Please see the `sleet` subdirectory for further information.

# hail

`hail` is a consensus algorithm based on `Snowman` but augmented with cryptographic sortition. It is specialised to blocks and ensures that no two conflicting blocks can be accepted at the same height. Similar to `sleet`, no inner verification of the block contents nor execution of state transitions is done besides on `alpha` primitive cells (such as staking cells).

`hail` passes blocks which are final on to a `block` recipient. The `block` recipient can be any type of `client` chain which serialised data into `cell`s a priori.

Please see the `hail` subdirectory for more information.

# alpha

`alpha` is the primary client chain of the `zfx` network. It defines the rules for executing the primitives of the network for staking and transferring capacity.

## Unified Overview

How the components fit together:
1. Ice performs a safe bootstrap with trusted peers and establishes liveness based on reservoir sampling consensus.
2. Once `ice` obtains sufficient live peers, the `alpha` chain state is bootstrapped and used to add sybil resistance to `ice` based on the latest validator set.
3. `sleet` consensus (mempool) is initialised with the latest validator set in order to query peers about transactions. The `alpha` frontier of final transactions is sent to `sleet` in order to provision the roots of new transactions.
4. Transactions are posted to `sleet` by the client in order to spend funds (e.g. sending from account A to B on the alpha chain). `sleet` resolves conflicts between these transactions, ensuring that only transactions which do not conflict (spend the same funds) eventually become final.
5. `hail` is initialised with the latest validator set in the same way as `sleet`. Whenever the VRF based selection selects the validator running `hail`, final transactions in `sleet` are used to generate a new block. `hail` resolves conflicts between blocks, ensuring that whenever a block conflicts at the same height the block with the lowest hash is selected.
6. A `block` recipient chain receives accepted blocks (final blocks) containing the cells that were finalised, executes the cells which are relevant to it and extends its blockchain.

## Node Identity

A node's identity is derived from its TLS certificate, and is verified for outgoing and incoming connections.

## Build, Run and test

Assuming that the standard set of Rust and C build tools are present, the following commands can be used to check out, build the project and run the tests:

```
git clone git@github.com:zfxlabs/zfx-subzero.git zfx-subzero
cd zfx-subzero
git checkout m3
cargo b
cargo t
```

### Running the local testnet

The local testnet is currently comprised of 3 nodes (for simplicity) which can be spawned by running the following commands run from the root of the Subzero repository:

```
cargo run --bin node -- -a 127.0.0.1:1234 -b 19Y53ymnBw4LWUpiAMUzPYmYqZmukRhNHm3VyAhzMqckRcuvkf@127.0.0.1:1235 --keypair ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416 --use-tls --cert-path deployment/test-certs/node0.crt -p deployment/test-certs/node0.key

cargo run --bin node -- -a 127.0.0.1:1235 -b 12My22AzQQosboCy6TCDFkTQwHTSuHhFN1VDcdDRPUe3H8j3DvY@127.0.0.1:1234 --keypair 5a353c630d3faf8e2d333a0983c1c71d5e9b6aed8f4959578fbeb3d3f3172886393b576de0ac1fe86a4dd416cf032543ac1bd066eb82585f779f6ce21237c0cd --use-tls --cert-path deployment/test-certs/node1.crt -p deployment/test-certs/node1.key

 cargo run --bin node -- -a 127.0.0.1:1236 -b 19Y53ymnBw4LWUpiAMUzPYmYqZmukRhNHm3VyAhzMqckRcuvkf@127.0.0.1:1235 --keypair 6f4b736b9a6894858a81696d9c96cbdacf3d49099d212213f5abce33da18716f067f8a2b9aeb602cd4163291ebbf39e0e024634f3be19bde4c490465d9095a6b --use-tls --cert-path deployment/test-certs/node2.crt -p deployment/test-certs/node2.key
```

There are scripts to simplify node startup in the [`deployment/scripts/`](deployment/scripts) and [`deployment/docker/`](deployment/docker) directories.
For more information, please refer [`deployment/README.md`](deployment/README.md).

### Running the client test

The client test which sends transactions in a loop to one of the validators mempool in the running local testnet can be executed with the following command, where the `--loop` argument can be used to control how many transactions get generated.

The testnet needs to be fully bootstrapped (Ice, Sleet and Hail initialised), in order to be able to accept transactions.

```
cargo run --bin client_test -- --peer 12My22AzQQosboCy6TCDFkTQwHTSuHhFN1VDcdDRPUe3H8j3DvY@127.0.0.1:1234 --keypair ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416 --cell-hash 9c486193789d15b66547157781519c734a46bb73b321ac5b1a187c11af1b61c9 --use-tls -p deployment/test-certs/test.key -c deployment/test-certs/test.crt --loop 16
```

### Accurate time source for node clock synchronization

To ensure proper operation of the nodes, the node must have an accurate time source by configuring a NTP/NTS daemon. NTS capable is recommended for maximum security.

## Documentation

There are individual README files in several subdirectories providing an overview of the given component.

For inline code documentation, run:

```
cargo doc --open
```

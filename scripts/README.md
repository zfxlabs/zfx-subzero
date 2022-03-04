# Test scripts

## `testnode.sh`

This script simplifies starting up a three-node testnet. The nodes use the certificates from the `test-certs/` directory, and their identity is currently part of the genesis.

```sh
zfx-subzero $ ./scripts/testnode.sh 0

zfx-subzero $ ./scripts/testnode.sh 1

zfx-subzero $ ./scripts/testnode.sh 2
```

By default, TLS connections are used. In order to set up a testnet with plain TCP connectivity, the `USE_TCP` environment varialble should be set:

```sh
zfx-subzero $ USE_TCP=1 ./scripts/testnode.sh 0

. . .
```

## `testclient.sh`

Sends transcation to a node in the tesnet. By default, a TLS connection is used, the `USE_TCP` environment variable can be used the same way as with the test nodes.

```sh
# Send a single transaction
zfx-subzero $ ./scripts/testclient.sh

# Send a multiple transactions
zfx-subzero $ ./scripts/testclient.sh --loop 11
```
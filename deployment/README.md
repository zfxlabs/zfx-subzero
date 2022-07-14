# Deployment of the zfx-subzero nodes

## Docker
Run the script `startup-nodes.sh` or `startup-nodes.bat`. It will build the image and start up the 3 nodes on docker containers.

_NOTE: the containers run nodes with TLS only, therefore if you use [`testclient.sh`](scripts/testclient.sh) to send some transactions, you **must not** use `USE_TCP=1` var._


## Test scripts

### `testnode.sh`

This script simplifies starting up a three-node testnet. The nodes use the certificates from the [`test-certs/`](test-certs) directory, 
and their identity is currently part of the genesis block.

```sh
zfx-subzero $ ./deployment/scripts/testnode.sh 0
zfx-subzero $ ./deployment/scripts/testnode.sh 1
zfx-subzero $ ./deployment/scripts/testnode.sh 2
```

By default, TLS connections are used. In order to set up a testnet with plain TCP connectivity, the `USE_TCP=1` environment variable should be set:

```sh
zfx-subzero $ USE_TCP=1 ./deployment/scripts/testnode.sh 0
. . .
```

### Accurate time source for node clock synchronization

To ensure proper operation of the nodes, the node must have an accurate time source by configuring a NTP/NTS daemon. NTS capable is recommended for maximum security.

### `testclient.sh`

This is a test script to sends some transactions to a node in the testnet.
By default, a TLS connection is used. The `USE_TCP` environment variable can be used the same way as when running the test nodes _(see above)_.

```sh
# Send a single transaction
zfx-subzero $ ./deployment/scripts/testclient.sh

# Send a multiple transactions
zfx-subzero $ ./deployment/scripts/testclient.sh --loop 11
```
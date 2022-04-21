# Deployment of the zfx-subzero nodes

## Docker
_(tested only on amd64 Win10 and Ubuntu)_

Run the script `startup-nodes.sh` or `startup-nodes.bat`. It will build the image and start up the nodes on docker containers.

_NOTE: the containers run nodes with TLS only, therefore if you use `testclient.sh` to send some transactions, you must not use `USE_TCP=1` var._


## Test scripts

## `testnode.sh`

This script simplifies starting up a three-node testnet. The nodes use the certificates from the `test-certs/` directory, and their identity is currently part of the genesis.

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

## `testclient.sh`

Sends transactions to a node in the tesnet. By default, a TLS connection is used, the `USE_TCP` environment variable can be used the same way as with the test nodes.

```sh
# Send a single transaction
zfx-subzero $ ./deployment/scripts/testclient.sh

# Send a multiple transactions
zfx-subzero $ ./deployment/scripts/testclient.sh --loop 11
```
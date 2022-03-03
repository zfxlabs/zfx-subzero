#!/bin/sh

set -ex

ZFX_ROOT="$(dirname $0)/../"

# Assume all operation happen in the repo root from now on
cd "$ZFX_ROOT"
pwd

if [[ -z "$USE_TCP" ]]; then
   # Use TLS

   cargo run --bin client_test -- \
       --peer 12My22AzQQosboCy6TCDFkTQwHTSuHhFN1VDcdDRPUe3H8j3DvY@127.0.0.1:1234 \
       --keypair ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416 \
       --cell-hash b5fba12b605e166987f031c300e33969e07e295285a3744692f326535fba555e \
       --use-tls -p test-certs/test.key -c test-certs/test.crt\
       "${@}"
else

    cargo run --bin client_test -- \
        --peer 12My22AzQQosboCy6TCDFkTQwHTSuHhFN1VDcdDRPUe3H8j3DvY@127.0.0.1:1234 \
        --keypair ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416 \
        --cell-hash b5fba12b605e166987f031c300e33969e07e295285a3744692f326535fba555e \
        "${@}"

fi
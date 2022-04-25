#!/bin/sh

set -ex

ZFX_ROOT="$(dirname $0)/../../"

# Assume all operation happen in the repo root from now on
cd "$ZFX_ROOT"
pwd

if [[ -z "$USE_TCP" ]]; then
    cargo run --bin client_test -- \
        --peer 12My22AzQQosboCy6TCDFkTQwHTSuHhFN1VDcdDRPUe3H8j3DvY@127.0.0.1:1234 \
        --keypair ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416 \
        --cell-hash 9c486193789d15b66547157781519c734a46bb73b321ac5b1a187c11af1b61c9 \
        "${@}"
else
   # Use TLS
   cargo run --bin client_test -- \
         --peer 12My22AzQQosboCy6TCDFkTQwHTSuHhFN1VDcdDRPUe3H8j3DvY@127.0.0.1:1234 \
         --keypair ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416 \
         --cell-hash 9c486193789d15b66547157781519c734a46bb73b321ac5b1a187c11af1b61c9 \
         --use-tls -p deployment/test-certs/test.key -c deployment/test-certs/test.crt \
         "${@}"
fi
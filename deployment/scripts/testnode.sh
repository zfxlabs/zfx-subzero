#!/bin/sh

set -ex

ZFX_ROOT="$(dirname $0)/../../"

# Assume all operation happen in the repo root from now on
cd "$ZFX_ROOT"
pwd

node0_id=12My22AzQQosboCy6TCDFkTQwHTSuHhFN1VDcdDRPUe3H8j3DvY
node1_id=19Y53ymnBw4LWUpiAMUzPYmYqZmukRhNHm3VyAhzMqckRcuvkf
node2_id=1A2iUK1VQWMfvtmrBpXXkVJjM5eMWmTfMEcBx4TatSJeuoSH7n

node0_ip=127.0.0.1:1234
node1_ip=127.0.0.1:1235
node2_ip=127.0.0.1:1236

keypair0=ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416
keypair1=5a353c630d3faf8e2d333a0983c1c71d5e9b6aed8f4959578fbeb3d3f3172886393b576de0ac1fe86a4dd416cf032543ac1bd066eb82585f779f6ce21237c0cd
keypair2=6f4b736b9a6894858a81696d9c96cbdacf3d49099d212213f5abce33da18716f067f8a2b9aeb602cd4163291ebbf39e0e024634f3be19bde4c490465d9095a6b

node0() {
   if [[ -z "$USE_TCP" ]]; then
        # Use TLS
        cargo run --bin node -- -a $node0_ip -b $node1_id@$node1_ip \
            --keypair $keypair0 \
            --use-tls --cert-path deployment/test-certs/node0.crt -p deployment/test-certs/node0.key
   else
        cargo run --bin node -- -a $node0_ip --id $node0_id -b $node1_id@$node1_ip \
            --keypair $keypair0
   fi
}

node1() {
   if [[ -z "$USE_TCP" ]]; then
        # Use TLS
        cargo run --bin node -- -a $node1_ip -b $node0_id@$node0_ip \
            --keypair $keypair1 \
            --use-tls --cert-path deployment/test-certs/node1.crt -p deployment/test-certs/node1.key     
   else
        cargo run --bin node -- -a $node1_ip --id $node1_id -b $node0_id@$node0_ip \
            --keypair $keypair1
       
   fi
}

node2() {
   if [[ -z "$USE_TCP" ]]; then
        # Use TLS
        cargo run --bin node -- -a $node2_ip -b $node1_id@$node1_ip \
            --keypair $keypair2 \
            --use-tls --cert-path deployment/test-certs/node2.crt -p deployment/test-certs/node2.key
   else
        cargo run --bin node -- -a $node2_ip --id $node2_id -b $node1_id@$node1_ip \
            --keypair $keypair2
   fi
}


case $1 in
    0) node0 ;;
    1) node1 ;;
    2) node2 ;;
    *)
        echo $0: bad argument
        exit 1
        ;;
esac

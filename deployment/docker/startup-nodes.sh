#!/bin/sh

set -ex

ZFX_ROOT="$(dirname $0)"
cd $ZFX_ROOT
ZFX_SCRIPTS_DIR="$(pwd $0)"

cd ../..
echo "Start to build the docker image for zfx-subzero node"
docker build -t zfx-subzero-node .

cd $ZFX_SCRIPTS_DIR
echo "Run zfx-subzero nodes"
docker-compose stop
docker-compose rm -f
docker-compose up
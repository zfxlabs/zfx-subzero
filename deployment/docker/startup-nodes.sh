#!/bin/sh

cd ../..
echo "Start to build the docker image for zfx-subzero node"
docker build -t zfx-performance-test-node .

cd deployment/docker || exit
echo "Run zfx-subzero nodes"
docker-compose stop
docker-compose rm -f
docker-compose up
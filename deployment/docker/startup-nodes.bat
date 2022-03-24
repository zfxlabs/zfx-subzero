cd ../..

echo "Start to build the docker image"
call docker build -t zfx-subzero-node .

cd deployment/docker
echo "Run zfx-subzero nodes"
call docker-compose stop
call docker-compose rm -f
call docker-compose up
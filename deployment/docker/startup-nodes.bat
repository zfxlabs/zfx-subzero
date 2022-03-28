SET ZFX_SCRIPTS_DIR=%~dp0

cd ../..
echo "Start to build the docker image for zfx-subzero node"
call docker build -t zfx-subzero-node .

cd %ZFX_SCRIPTS_DIR%
echo "Run zfx-subzero nodes"
call docker-compose stop
call docker-compose rm -f
call docker-compose up
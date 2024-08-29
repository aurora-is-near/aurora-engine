NETWORK=`echo "${1:-mainnet}" | cut -d '-' -f 1`
TAG=`curl -s https://rpc.${NETWORK}.near.org/status  | jq -r .version.build`

git clone https://github.com/near/nearcore -b $TAG
cd nearcore
make sandbox-release
cd ..
mv ${PWD}/nearcore/target/release/neard-sandbox .
rm -rf nearcore
export NEAR_SANDBOX_BIN_PATH=${PWD}/neard-sandbox

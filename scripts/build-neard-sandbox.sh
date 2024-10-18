NETWORK=`echo "${1:-mainnet}" | cut -d '-' -f 1`
TAG=`curl -s https://rpc.${NETWORK}.near.org/status  | jq -r .version.build`

git clone https://github.com/near/nearcore -b $TAG
cd nearcore
make sandbox-release
mv $(find target -name neard-sandbox) ../
cd ..
rm -rf nearcore
echo "NEAR_SANDBOX_BIN_PATH=${PWD}/neard-sandbox" >> $GITHUB_ENV

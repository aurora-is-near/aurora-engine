#!/bin/sh -e

if [ $# -ne 1 ]
then
  echo "Usage: $0 [-goerli | -ropsten | -rinkeby]" >&2
  exit 1
fi

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
CONTRACT_PATH=$SCRIPT_DIR/../mainnet-release.wasm

export NEAR_ENV=local

cd $SCRIPT_DIR/workdir/evm-bully
./evm-bully -v replay \
    -initial-balance 1000 \
    -keyPath $HOME/.near/local/validator_key.json \
    -autobreak -setup -skip $1 -contract $CONTRACT_PATH

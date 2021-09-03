#!/bin/bash -e


# Validating arguments

if [ $# -ne 1 ]
then
  echo "Usage: $0 [-goerli | -ropsten | -rinkeby]" >&2
  exit 1
fi


# Determining paths

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
CONTRACT_PATH=$SCRIPT_DIR/../mainnet-release.wasm
STATE_PATH=$SCRIPT_DIR/state.json


# Running bully

cd $SCRIPT_DIR/workdir/evm-bully
rm -f *.tar.gz

export NEAR_ENV=local
./evm-bully -v replay \
    -initial-balance 1000 \
    -keyPath $HOME/.near/local/validator_key.json \
    -autobreak -setup -skip $1 -contract $CONTRACT_PATH


# Examining results

TESTNET='goerli' # HARDCODED temporary
ARTIFACT=$(ls | grep tar | grep $TESTNET | head -1)
EXPECTED_ARTIFACT=$(jq -r '.progress.goerli' $STATE_PATH)

if [[ $ARTIFACT == "" ]]; then
    echo "Bully results are empty"
    exit 1
fi

if [[ $ARTIFACT == $EXPECTED_ARTIFACT ]]; then
    echo "Success"
else
    echo "Expected artifact was $EXPECTED_ARTIFACT, but $ARTIFACT was found"
    exit 1
fi

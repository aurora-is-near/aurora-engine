#!/bin/bash -e


# Load settings

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
source $SCRIPT_DIR/common.sh


# Validate arguments

if [ $# -ne 1 ]
then
  echo "Usage: $0 [goerli | ropsten | rinkeby]" >&2
  exit 1
fi

TESTNET=$1


# Run bully

cd $WORKDIR/evm-bully
rm -f *.tar.gz

export NEAR_ENV=local
./evm-bully -v replay \
    -initial-balance 1000 \
    -keyPath $HOME/.near/local/validator_key.json \
    -neard $NEARD_PATH \
    -neardhead $NEARCORE_VERSION \
    -auroracli $AURORA_CLI_PATH \
    -autobreak -setup -skip -$TESTNET -contract $CONTRACT_PATH


# Examining results

ARTIFACT=$(ls | grep tar | grep $TESTNET | head -1)
EXPECTED_ARTIFACT=$(jq -r --arg testnet "${TESTNET}" '.progress[$testnet]' $STATE_PATH)

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

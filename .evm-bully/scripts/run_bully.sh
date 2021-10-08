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

cd $WORKDIR
rm -f *.tar.gz

export NEAR_ENV=local
$EVM_BULLY_BINARY_PATH -v replay \
    -initial-balance 1000 \
    -keyPath $HOME/.near/local/validator_key.json \
    -neard $NEARCORE_BINARY_PATH \
    -neardhead $NEARCORE_VERSION \
    -auroracli $AURORA_CLI_PATH \
    -autobreak -setup -skip -$TESTNET -contract $AURORA_ENGINE_BINARY_PATH


# Examine results

ARTIFACT=$(ls | grep tar | grep $TESTNET | head -1)
EXPECTED_ARTIFACT=$(jq -r --arg testnet "${TESTNET}" '.progress[$testnet]' $STATE_PATH)

echo "Found output artifact: $ARTIFACT"
echo "Expected: $EXPECTED_ARTIFACT"


RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color


if [[ $ARTIFACT = "" ]]; then
    echo -e "${RED}Bully results are empty${NC}"
    exit 1
fi

if [[ $ARTIFACT = $EXPECTED_ARTIFACT ]]; then
    echo -e "${GREEN}Success, nothing changed${NC}"
else
    BLOCK_NUM=$(echo $ARTIFACT | grep -o -E '[0-9]+' | head -1)
    TX_NUM=$(echo $ARTIFACT | grep -o -E '[0-9]+' | tail -1)
    EXPECTED_BLOCK_NUM=$(echo $EXPECTED_ARTIFACT | grep -o -E '[0-9]+' | head -1)
    EXPECTED_TX_NUM=$(echo $EXPECTED_ARTIFACT | grep -o -E '[0-9]+' | tail -1)

    BULLY_PROGRESS=0
    if [[ $BLOCK_NUM -gt $EXPECTED_BLOCK_NUM ]]; then
        BULLY_PROGRESS=1
    elif [[ $BLOCK_NUM -lt $EXPECTED_BLOCK_NUM ]]; then
        BULLY_PROGRESS=-1
    elif [[ $TX_NUM -gt $EXPECTED_TX_NUM ]]; then
        BULLY_PROGRESS=1
    elif [[ $TX_NUM -lt $EXPECTED_TX_NUM ]]; then
        BULLY_PROGRESS=-1
    fi

    if [[ $BULLY_PROGRESS = -1 ]]; then
        echo -e "${RED}Downgrade detected.${NC}"
        exit 1
    elif [[ $BULLY_PROGRESS = 1 ]]; then
        echo -e "${YELLOW}Progress detected. Please update state.json with new results${NC}"
        exit 1
    elif [[ $BULLY_PROGRESS = 0 ]]; then
        echo -e "${YELLOW}Can't parse block/tx numbers from artifact${NC}"
    fi
fi

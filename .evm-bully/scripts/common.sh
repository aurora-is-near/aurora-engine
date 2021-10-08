#!/bin/bash -e

checkout_repo() {
    if [[ ! -d $1/.git ]]; then
        mkdir -p $1
        git clone $2 $1
    fi
    cd $1
    git fetch
    git reset --hard -q $3
}

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
STATE_PATH=$SCRIPT_DIR/../state.json
WORKDIR=$SCRIPT_DIR/../workdir

AURORA_CLI_VERSION=$(jq -r '."version-lock"."aurora-cli"' $STATE_PATH)
AURORA_CLI_REPO_PATH=$WORKDIR/aurora-cli
AURORA_CLI_PATH=$AURORA_CLI_REPO_PATH/lib/aurora.js

cd $SCRIPT_DIR/../../
AURORA_ENGINE_VERSION=$(git rev-parse HEAD)
AURORA_ENGINE_CACHE_KEY=aurora-engine-$AURORA_ENGINE_VERSION
AURORA_ENGINE_BINARY_PATH=$WORKDIR/$AURORA_ENGINE_CACHE_KEY

EVM_BULLY_VERSION=$(jq -r '."version-lock"."evm-bully"' $STATE_PATH)
EVM_BULLY_CACHE_KEY=evm-bully-$EVM_BULLY_VERSION
EVM_BULLY_BINARY_PATH=$WORKDIR/$EVM_BULLY_CACHE_KEY
EVM_BULLY_REPO_PATH=$WORKDIR/evm-bully

NEARCORE_VERSION=$(jq -r '."version-lock"."nearcore"' $STATE_PATH)
NEARCORE_CACHE_KEY=nearcore-$NEARCORE_VERSION
NEARCORE_BINARY_PATH=$WORKDIR/$NEARCORE_CACHE_KEY
NEARCORE_REPO_PATH=$WORKDIR/nearcore
DOWNLOAD_NEARCORE=true

CUSTOM_CONFIG_PATH=$SCRIPT_DIR/../config.sh
if [[ -f $CUSTOM_CONFIG_PATH ]]; then
    source $CUSTOM_CONFIG_PATH
fi

mkdir -p $WORKDIR
mkdir -p ~/.near-credentials/local

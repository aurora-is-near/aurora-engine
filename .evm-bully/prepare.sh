#!/bin/bash -e


# Determine paths

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
STATE_PATH=$SCRIPT_DIR/state.json
WORKDIR=$SCRIPT_DIR/workdir

mkdir -p $WORKDIR
mkdir -p ~/.near-credentials/local


# Read versions

AURORA_CLI_VERSION=$(jq -r '."version-lock"."aurora-cli"' $STATE_PATH)
NEARCORE_VERSION=$(jq -r '."version-lock"."nearcore"' $STATE_PATH)
EVM_BULLY_VERSION=$(jq -r '."version-lock"."evm-bully"' $STATE_PATH)


# Checkout

npm install -g "git://github.com/aurora-is-near/aurora-cli.git#$AURORA_CLI_VERSION"

cd $WORKDIR && git clone https://github.com/near/nearcore.git
cd $WORKDIR/nearcore && git checkout $NEARCORE_VERSION

cd $WORKDIR && git clone https://github.com/aurora-is-near/evm-bully.git
cd $WORKDIR/evm-bully && git checkout $EVM_BULLY_VERSION


# Build bully

cd $WORKDIR/evm-bully && make

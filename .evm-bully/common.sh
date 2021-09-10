#!/bin/bash -e


# Determine paths

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
STATE_PATH=$SCRIPT_DIR/state.json
WORKDIR=$SCRIPT_DIR/workdir
NEARD_PATH=$WORKDIR/neard
AURORA_CLI_PATH=$WORKDIR/aurora-cli/lib/aurora.js
CONTRACT_PATH=$SCRIPT_DIR/../mainnet-release.wasm

# Read state

AURORA_CLI_VERSION=$(jq -r '."version-lock"."aurora-cli"' $STATE_PATH)
NEARCORE_VERSION=$(jq -r '."version-lock"."nearcore"' $STATE_PATH)
EVM_BULLY_VERSION=$(jq -r '."version-lock"."evm-bully"' $STATE_PATH)


# Create directories

mkdir -p $WORKDIR
mkdir -p ~/.near-credentials/local

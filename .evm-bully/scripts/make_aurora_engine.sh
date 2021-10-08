#!/bin/bash -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
source $SCRIPT_DIR/common.sh


if [[ ! -f $AURORA_ENGINE_BINARY_PATH ]] && [[ ! -z $USE_CACHE_UTIL ]]; then
    echo "aurora-engine: trying to restore from cache..."
    cache-util restore $AURORA_ENGINE_CACHE_KEY:$AURORA_ENGINE_BINARY_PATH || true
    if [[ -f $AURORA_ENGINE_BINARY_PATH ]]; then
        exit 0
    fi
fi

if [[ ! -f $AURORA_ENGINE_BINARY_PATH ]]; then
    echo "aurora-engine: building..."
    cd $SCRIPT_DIR/../..
    make evm-bully=yes
    cp ./mainnet-release.wasm $AURORA_ENGINE_BINARY_PATH
fi

if [[ -f $AURORA_ENGINE_BINARY_PATH ]] && [[ ! -z $USE_CACHE_UTIL ]]; then
    echo "aurora-engine: saving to cache..."
    cache-util save $AURORA_ENGINE_CACHE_KEY:$AURORA_ENGINE_BINARY_PATH || true
fi

#!/bin/bash -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
source $SCRIPT_DIR/common.sh


if [[ ! -f $EVM_BULLY_BINARY_PATH ]] && [[ ! -z $USE_CACHE_UTIL ]]; then
    echo "evm-bully: trying to restore from cache..."
    cache-util restore $EVM_BULLY_CACHE_KEY:$EVM_BULLY_BINARY_PATH || true
    if [[ -f $EVM_BULLY_BINARY_PATH ]]; then
        exit 0
    fi
fi

if [[ ! -f $EVM_BULLY_BINARY_PATH ]]; then
    echo "evm-bully: checkouting repo..."
    checkout_repo $EVM_BULLY_REPO_PATH https://github.com/aurora-is-near/evm-bully.git $EVM_BULLY_VERSION

    echo "evm-bully: building..."
    cd $EVM_BULLY_REPO_PATH
    make
    cp ./evm-bully $EVM_BULLY_BINARY_PATH
fi

if [[ -f $EVM_BULLY_BINARY_PATH ]] && [[ ! -z $USE_CACHE_UTIL ]]; then
    echo "evm-bully: saving to cache..."
    cache-util save $EVM_BULLY_CACHE_KEY:$EVM_BULLY_BINARY_PATH || true
fi

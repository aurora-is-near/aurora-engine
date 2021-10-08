#!/bin/bash -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
source $SCRIPT_DIR/common.sh


if [[ ! -f $NEARCORE_BINARY_PATH ]] && [[ ! -z $USE_CACHE_UTIL ]]; then
    echo "nearcore: trying to restore from cache..."
    cache-util restore $NEARCORE_CACHE_KEY:$NEARCORE_BINARY_PATH || true
    if [[ -f $NEARCORE_BINARY_PATH ]]; then
        exit 0
    fi
fi

if [[ ! -f $NEARCORE_BINARY_PATH ]] && [[ $DOWNLOAD_NEARCORE = true ]]; then
    echo "nearcore: trying to download..."
    curl -L \
        https://s3.us-west-1.amazonaws.com/build.nearprotocol.com/nearcore/$(uname)/master/${NEARCORE_VERSION}/neard \
        -o $NEARCORE_BINARY_PATH \
        || true
    chmod +x $NEARCORE_BINARY_PATH || true
fi

if [[ ! -f $NEARCORE_BINARY_PATH ]]; then
    echo "nearcore: checkouting repo..."
    checkout_repo $NEARCORE_REPO_PATH https://github.com/near/nearcore.git $NEARCORE_VERSION

    echo "nearcore: building..."
    cd $NEARCORE_REPO_PATH
    cargo build --package neard --features nightly_protocol_features --release
    cp target/release/neard $NEARCORE_BINARY_PATH
fi

if [[ -f $NEARCORE_BINARY_PATH ]] && [[ ! -z $USE_CACHE_UTIL ]]; then
    echo "nearcore: saving to cache..."
    cache-util save $NEARCORE_CACHE_KEY:$NEARCORE_BINARY_PATH || true
fi

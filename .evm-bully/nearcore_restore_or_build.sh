#!/bin/bash -e


# Load settings

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
source $SCRIPT_DIR/common.sh


# Try restore from cache

rm -rf $NEARD_PATH
cache-util restore neard-$NEARCORE_VERSION:$NEARD_PATH
if [[ -f $NEARD_PATH ]]; then
    echo "neard with git hash $NEARCORE_VERSION was successfully restored from cache"
    exit 0
fi

echo "neard with git hash $NEARCORE_VERSION was not restored from cache, building..."


# Checkout and build

rm -rf $WORKDIR/nearcore
cd $WORKDIR && git clone https://github.com/near/nearcore.git
cd $WORKDIR/nearcore && git checkout $NEARCORE_VERSION
cargo build --package neard --features nightly_protocol_features --release
mv $WORKDIR/nearcore/target/release/neard $NEARD_PATH

#!/bin/bash -e


# Load settings

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
source $SCRIPT_DIR/common.sh


# Checkout and build

rm -rf $WORKDIR/evm-bully
cd $WORKDIR && git clone https://github.com/aurora-is-near/evm-bully.git
cd $WORKDIR/evm-bully && git checkout $EVM_BULLY_VERSION
make

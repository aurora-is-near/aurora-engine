#!/bin/bash -e


# Load settings

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
source $SCRIPT_DIR/common.sh


# Checkout and install dependencies

rm -rf $WORKDIR/aurora-cli
cd $WORKDIR && git clone https://github.com/aurora-is-near/aurora-cli.git
cd $WORKDIR/aurora-cli && git checkout $AURORA_CLI_VERSION
npm install

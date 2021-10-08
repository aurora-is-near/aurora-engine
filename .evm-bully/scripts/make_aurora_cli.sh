#!/bin/bash -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
source $SCRIPT_DIR/common.sh


if [[ ! -z $AURORA_CLI_PREVENT_CHECKOUT ]]; then
    echo "aurora-cli: checkouting repo..."
    checkout_repo $AURORA_CLI_REPO_PATH https://github.com/aurora-is-near/aurora-cli.git $AURORA_CLI_VERSION
fi

echo "aurora-cli: npm install..."
cd $AURORA_CLI_REPO_PATH
npm install

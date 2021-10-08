#!/bin/bash -e

if [ $# -ne 1 ]
then
  echo "Usage: $0 [goerli | ropsten | rinkeby]" >&2
  exit 1
fi

CURRENT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
$CURRENT_DIR/scripts/make_aurora_cli.sh
$CURRENT_DIR/scripts/make_aurora_engine.sh
$CURRENT_DIR/scripts/make_evm_bully.sh
$CURRENT_DIR/scripts/make_nearcore.sh
$CURRENT_DIR/scripts/run_bully.sh $1

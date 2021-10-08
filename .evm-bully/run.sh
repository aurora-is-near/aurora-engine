#!/bin/bash -e

CURRENT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
$CURRENT_DIR/scripts/make_aurora_cli.sh
$CURRENT_DIR/scripts/make_aurora_engine.sh
$CURRENT_DIR/scripts/make_evm_bully.sh
$CURRENT_DIR/scripts/make_nearcore.sh
$CURRENT_DIR/scripts/run_bully.sh

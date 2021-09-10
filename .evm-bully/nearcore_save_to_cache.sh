#!/bin/bash -e


# Load settings

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
source $SCRIPT_DIR/common.sh


# Save neard to cache

cache-util msave neard-$NEARCORE_VERSION:$NEARD_PATH

#!/bin/bash

export DEBIAN_FRONTEND=noninteractive

apt update
apt install -y build-essential pkg-config libclang-dev libssl-dev gnupg curl git gpg
scripts/ci-deps/install-wasm-opt.sh

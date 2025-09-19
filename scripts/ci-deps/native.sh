#!/bin/bash

export DEBIAN_FRONTEND=noninteractive

apt update
apt install -y build-essential pkg-config libclang-dev libssl-dev gnupg curl git
curl -o- -L https://yarnpkg.com/install.sh | bash
yarn install
scripts/ci-deps/install-wasm-opt.sh

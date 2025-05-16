#!/bin/bash

export DEBIAN_FRONTEND=noninteractive

apt update
apt install -y build-essential pkg-config libclang-dev libssl-dev gnupg curl git
curl -sS https://dl.yarnpkg.com/debian/pubkey.gpg | apt-key add -
echo "deb https://dl.yarnpkg.com/debian/ stable main" | tee /etc/apt/sources.list.d/yarn.list
apt update
apt install -y yarn
yarn install
scripts/ci-deps/install-wasm-opt.sh

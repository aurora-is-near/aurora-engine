#!/bin/bash

export DEBIAN_FRONTEND=noninteractive

apt update
apt install -y build-essential pkg-config libclang-dev libssl-dev gnupg curl git gpg

mkdir -p /etc/apt/keyrings
curl -fsSL https://dl.yarnpkg.com/debian/pubkey.gpg | gpg --dearmor -o /etc/apt/keyrings/yarn.gpg
chmod a+r /etc/apt/keyrings/yarn.gpg
echo "deb [signed-by=/etc/apt/keyrings/yarn.gpg] https://dl.yarnpkg.com/debian stable main" \
  | tee /etc/apt/sources.list.d/yarn.list >/dev/null
apt install -y yarn
yarn install
scripts/ci-deps/install-wasm-opt.sh

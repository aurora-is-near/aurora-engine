#!/bin/bash

export DEBIAN_FRONTEND=noninteractive

apt update
apt install -y build-essential pkg-config libclang-dev libssl-dev gnupg curl git gpg

mkdir -p /etc/apt/keyrings
curl -sS https://dl.yarnpkg.com/debian/pubkey.gpg \
  | gpg --dearmor \
  | tee /etc/apt/keyrings/yarn-archive-keyring.gpg > /dev/null
echo "deb [signed-by=/etc/apt/keyrings/yarn-archive-keyring.gpg] https://dl.yarnpkg.com/debian/ stable main" \
  | tee /etc/apt/sources.list.d/yarn.list

apt update
apt install -y yarn
yarn install
scripts/ci-deps/install-wasm-opt.sh

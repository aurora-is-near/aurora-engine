#!/bin/bash

export DEBIAN_FRONTEND=noninteractive

apt update
apt install -y build-essential pkg-config libclang-dev libssl-dev gnupg curl git
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
\. "$HOME/.nvm/nvm.sh"
nvm install 18
npm install --global yarn
yarn install
scripts/ci-deps/install-wasm-opt.sh

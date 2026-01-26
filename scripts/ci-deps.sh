#!/bin/bash

export DEBIAN_FRONTEND=noninteractive
BINARYEN_VERSION=125

apt update
apt install -y build-essential pkg-config libclang-dev libssl-dev gnupg curl git gpg


if [[ ! -f wasm-opt ]]; then
  mkdir binaryen
  curl -sL https://github.com/WebAssembly/binaryen/releases/download/version_$BINARYEN_VERSION/binaryen-version_$BINARYEN_VERSION-x86_64-linux.tar.gz | tar -xz -C binaryen
  cp binaryen/binaryen-version_$BINARYEN_VERSION/bin/* /usr/local/bin
  rm -rf binaryen
fi

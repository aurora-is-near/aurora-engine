#!/usr/bin/env bash

VERSION=119

if [[ ! -f wasm-opt ]]; then
  mkdir binaryen
  curl -sL https://github.com/WebAssembly/binaryen/releases/download/version_$VERSION/binaryen-version_$VERSION-x86_64-linux.tar.gz | tar -xz -C binaryen
  cp binaryen/binaryen-version_$VERSION/bin/* /usr/local/bin
  rm -rf binaryen
fi

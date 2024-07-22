#!/usr/bin/env bash

if [[ ! -f wasm-opt ]]; then
  mkdir binaryen
  curl -L https://github.com/WebAssembly/binaryen/releases/download/version_110/binaryen-version_110-x86_64-linux.tar.gz | tar -xz -C binaryen
  cp binaryen/binaryen-version_110/bin/* /usr/local/bin
  rm -rf binaryen
fi

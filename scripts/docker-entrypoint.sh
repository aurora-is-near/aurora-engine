#!/bin/bash
rustup toolchain add stable
cargo +stable install --no-default-features --force cargo-make
scripts/ci/install-wasm-opt.sh
cargo make --profile "$1" build-docker-inner

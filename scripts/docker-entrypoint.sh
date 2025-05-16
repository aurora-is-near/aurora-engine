#!/bin/bash
rustup toolchain add stable
cargo +stable install --no-default-features --force cargo-make
scripts/ci-deps/install-wasm-opt.sh
cargo make build-docker-inner

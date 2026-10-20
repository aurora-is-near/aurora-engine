#!/bin/bash
set -euo pipefail
rustup toolchain add stable
cargo +stable install --no-default-features --locked --version "${CARGO_MAKE_VERSION:?Run cargo make build-docker}" --force cargo-make
scripts/ci-deps.sh
cargo make build-docker-inner

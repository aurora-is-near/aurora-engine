#!/bin/bash
rustup toolchain add stable
cargo +stable install --no-default-features --force cargo-make
cargo make --profile "$1" build-xcc-router-docker-inner

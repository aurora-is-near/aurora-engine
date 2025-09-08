#!/bin/env bash
set -e

sed -i 's/pub const fn \(increment\|decrement\)/pub fn \1/' engine-standalone-tracing/src/types/mod.rs
cargo +nightly-2024-05-02 build --target wasm32-unknown-unknown --release --features=contract_3_7_0 --package=aurora-engine-compat -Zbuild-std
wasm-opt -O4 target/wasm32-unknown-unknown/release/aurora_engine_compat.wasm -o bin/aurora-engine-3.7.0.wasm --strip-debug --vacuum
sed -i 's/pub fn \(increment\|decrement\)/pub const fn \1/' engine-standalone-tracing/src/types/mod.rs
cargo build --target wasm32-unknown-unknown --release --features=contract_3_9_0 --package=aurora-engine-compat
wasm-opt -O4 target/wasm32-unknown-unknown/release/aurora_engine_compat.wasm -o bin/aurora-engine-3.9.0.wasm --strip-debug --vacuum

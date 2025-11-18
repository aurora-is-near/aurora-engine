# Build

```
cargo build --target wasm32-unknown-unknown --release --features=contract --package=aurora-engine-compat
wasm-opt -O4 target/wasm32-unknown-unknown/release/aurora_engine_compat.wasm -o bin/aurora-engine.wasm --strip-debug --vacuum
```

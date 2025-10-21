# Backport

```
git checkout tags/3.6.4
git cherry-pick 8734db40090b29c15e9d60b91ec9c041aef8def5
# resolve conflicts
git checkout -b tracing/3.6.4
cargo build --target wasm32-unknown-unknown --package=aurora-engine-compat --features=contract --release
wasm-opt -O4 target/wasm32-unknown-unknown/release/aurora_engine_compat.wasm -o ../borealis-rs/etc/res/aurora-engine-3.6.4.wasm --strip-debug --vacuum
cargo build --target wasm32-unknown-unknown --package=aurora-engine-compat --features=contract,ext-connector --release
wasm-opt -O4 target/wasm32-unknown-unknown/release/aurora_engine_compat.wasm -o ../borealis-rs/etc/res/aurora-engine-3.6.4-ext-connector.wasm --strip-debug --vacuum
```

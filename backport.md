# Backport

```
git checkout tags/3.6.4
git cherry-pick 8734db40090b29c15e9d60b91ec9c041aef8def5
# resolve conflicts
git checkout -b tracing/3.6.4
cargo build --target wasm32-unknown-unknown --package=aurora-engine-compat --features=contract --release
cp target/wasm32-unknown-unknown/release/aurora_engine_compat.wasm ../borealis-rs/etc/res/aurora-engine-3.6.4.wasm
cargo build --target wasm32-unknown-unknown --package=aurora-engine-compat --features=contract,ext-connector --release
cp target/wasm32-unknown-unknown/release/aurora_engine_compat.wasm ../borealis-rs/etc/res/aurora-engine-3.6.4-ext-connector.wasm
```

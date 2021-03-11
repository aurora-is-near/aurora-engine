# Aurora Engine

[![Project license](https://img.shields.io/badge/license-Public%20Domain-blue.svg)](https://creativecommons.org/publicdomain/zero/1.0/)
[![Discord](https://img.shields.io/discord/490367152054992913?label=discord)](https://discord.gg/jNjHYUF8vw)
[![Lints](https://github.com/aurora-is-near/aurora-engine/actions/workflows/lints.yml/badge.svg)](https://github.com/aurora-is-near/aurora-engine/actions/workflows/lints.yml)

## Prerequisites

- Rust nightly (2021-01-30) with the WebAssembly toolchain
- GNU Make (3.81+)

```sh
rustup install nightly-2021-01-30
rustup target add wasm32-unknown-unknown --toolchain nightly-2021-01-30
```

## Development

### Building the contract

```sh
make release  # produces release.wasm
```

### Deploying the contract

```sh
export NEAR_ENV=local
near delete evm.test.near test.near  # if needed
near create-account evm.test.near --master-account=test.near --initial-balance 100000
near deploy --account-id=evm.test.near --wasm-file=release.wasm
```

### Calling the contract

```sh
near call evm.test.near get_version --account-id evm.test.near
```

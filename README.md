# Aurora Engine

[![Project license](https://img.shields.io/badge/license-Public%20Domain-blue.svg)](https://creativecommons.org/publicdomain/zero/1.0/)
[![Discord](https://img.shields.io/discord/490367152054992913?label=discord)](https://discord.gg/jNjHYUF8vw)
[![Lints](https://github.com/aurora-is-near/aurora-engine/actions/workflows/lints.yml/badge.svg)](https://github.com/aurora-is-near/aurora-engine/actions/workflows/lints.yml)
[![Tests](https://github.com/aurora-is-near/aurora-engine/actions/workflows/tests.yml/badge.svg)](https://github.com/aurora-is-near/aurora-engine/actions/workflows/tests.yml)
[![Builds](https://github.com/aurora-is-near/aurora-engine/actions/workflows/builds.yml/badge.svg)](https://github.com/aurora-is-near/aurora-engine/actions/workflows/builds.yml)

## Prerequisites

### Prerequisites for Building

- Rust nightly (2021-01-30) with the WebAssembly toolchain
- GNU Make (3.81+)

```sh
rustup install nightly-2021-01-30
rustup target add wasm32-unknown-unknown --toolchain nightly-2021-01-30
```

### Prerequisites for Deploying

- Node.js (v14+)

## Development

### Building the contract

```sh
make release  # produces release.wasm (300+ KiB)
make debug    # produces debug.wasm (1+ MiB), which includes symbols
```

### Running unit tests

```sh
make check
```

## Deployment

### Deploying the contract

```sh
export NEAR_ENV=local
near delete evm.test.near test.near  # if needed
near create-account evm.test.near --master-account=test.near --initial-balance 100000
near deploy --account-id=evm.test.near --wasm-file=release.wasm
node scripts/deploy.js -d
```

## Usage

### Calling the contract

```sh
near call evm.test.near get_version --account-id evm.test.near
near call evm.test.near get_owner --account-id evm.test.near
near call evm.test.near get_bridge_provider --account-id evm.test.near
near call evm.test.near get_chain_id --account-id evm.test.near
```

## Debugging

### Inspecting the contract state

```sh
near state evm.test.near
http post http://localhost:3030 jsonrpc=2.0 id=1 method=query params:='{"request_type": "view_state", "account_id": "evm.test.near", "prefix_base64": "", "finality": "final"}'
```

If you have [Ruby] installed, get more useful and readable output as follows:

```sh
rake dump
```

[Ruby]: https://www.ruby-lang.org

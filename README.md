# Aurora Engine

[![Project license](https://img.shields.io/badge/License-Public%20Domain-blue.svg)](https://creativecommons.org/publicdomain/zero/1.0/)
[![Discord](https://img.shields.io/discord/490367152054992913?label=Discord)](https://discord.gg/jNjHYUF8vw)
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

### Prerequisites for Development

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

### Installing the CLI

```sh
npm install -g aurora-is-near/aurora-cli
```

### Deploying the contract

```sh
export NEAR_ENV=local
near delete aurora.test.near test.near  # if needed
near create-account aurora.test.near --master-account=test.near --initial-balance 100000
near deploy --account-id=aurora.test.near --wasm-file=release.wasm
aurora init --chain 1313161556 --owner test.near
```

## Usage

### Calling the contract

```sh
aurora get-version
aurora get-owner
aurora get-bridge-provider
aurora get-chain-id
```

## Debugging

### Inspecting the contract state

```sh
near state aurora.test.near
aurora dump-storage
```

## Networks

Network | Chain ID
------- | ----------
BetaNet | 1313161556
TestNet | 1313161555
MainNet | 1313161554

## Interface

### Administrative methods

#### `new`

#### `get_version`

#### `get_owner`

#### `get_bridge_provider`

#### `get_chain_id`

#### `get_upgrade_index`

#### `stage_upgrade`

#### `deploy_upgrade`

### Mutative methods

#### `deploy_code`

#### `call`

#### `raw_call`

#### `meta_call`

### Nonmutative methods

#### `view`

#### `get_code`

#### `get_balance`

#### `get_nonce`

#### `get_storage_at`

### Benchmarking methods

#### `begin_chain`

#### `begin_block`

[Ruby]: https://www.ruby-lang.org

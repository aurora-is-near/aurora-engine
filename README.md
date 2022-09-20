# Aurora Engine

[![Project license](https://img.shields.io/badge/License-Public%20Domain-blue.svg)](https://creativecommons.org/publicdomain/zero/1.0/)
[![Discord](https://img.shields.io/discord/490367152054992913?label=Discord)](https://discord.gg/jNjHYUF8vw)
[![Lints](https://github.com/aurora-is-near/aurora-engine/actions/workflows/lints.yml/badge.svg)](https://github.com/aurora-is-near/aurora-engine/actions/workflows/lints.yml)
[![Tests](https://github.com/aurora-is-near/aurora-engine/actions/workflows/tests.yml/badge.svg)](https://github.com/aurora-is-near/aurora-engine/actions/workflows/tests.yml)
[![Builds](https://github.com/aurora-is-near/aurora-engine/actions/workflows/builds.yml/badge.svg)](https://github.com/aurora-is-near/aurora-engine/actions/workflows/builds.yml)

Aurora Engine implements an Ethereum Virtual Machine (EVM) on the NEAR Protocol.
See [doc.aurora.dev](https://doc.aurora.dev/develop/compat/evm) for additional
documentation.

## Deployments

Network | Contract ID         | Chain ID   | Version
------- | ------------------- | ---------- | ------
Mainnet | [`aurora`][Mainnet] | 1313161554 | 2.6.1
Testnet | [`aurora`][Testnet] | 1313161555 | 2.6.1
Local   | `aurora.test.near`  | 1313161556 | 2.6.1

[Mainnet]: https://explorer.near.org/accounts/aurora
[Testnet]: https://explorer.testnet.near.org/accounts/aurora

## Development

### Prerequisites

- Node.js (v14+)
- Rust nightly (2021-03-25) with the WebAssembly toolchain
- cargo-make

```sh
rustup install nightly-2021-03-25
rustup target add wasm32-unknown-unknown --toolchain nightly-2021-03-25
cargo install --force cargo-make
```

### Prerequisites for Development

- Node.js (v14+)
- Docker
- cargo-make

## Development

### Branches

- [`master`] is the current stable branch.
  It must be ready, anytime, to deployed on chain at a moment's notice.

- [`develop`] is our bleeding-edge development branch.
  In general, kindly target all pull requests to this branch.

### Building & Make Commands

Every task with `cargo make` must have a `--profile` argument.

The current available `profile`s are:
- `mainnet`, suitable for mainnet.
- `testnet`, suitable for testnet.
- `local`, suitable for local development.
- `custom`, suitable for custom environments, see note below.

A custom environment may be required depending on the circumstances. This can
be created in the `.env` folder as `custom.env` following the structure of the
other `.env` files. See `bin/local-custom.env` for more details.

Every make most follow the following pattern, though `--profile` is not required
for all such as cleanup:
```sh
cargo make [--profile <profile>] <task>
```

#### Building the engine and contracts

To build the binaries there are a few commands to do such following the format.

The current available build `task`s are:
- `default`, does not need to be specified, runs `build`. Requires a `--profile`
  argument.
- `build`, builds all engine smart contract and produces the
  `aurora-<profile>-test.wasm` in the `bin` folder. Requires `build-contracts`. 
  Requires a `--profile` argument.
- `build-test`, builds all the below using test features. Requires a `--profile`
  argument.
- `build-contracts`, builds all the ETH contracts.

For example, the following will build the mainnet debug binary:
```sh
cargo make --profile mainnet build
```

#### Running unit & integration tests

To run tests, there are a few cargo make tasks we can run:
- `test`, tests the whole cargo workspace and ETH contracts. Requires a 
  `--profile` argument.
- `test-workspace`, tests only the cargo workspace.
- `test-contracts`, tests only the contracts.

For example, the following will test the whole workspace and ETH contracts:
```sh
cargo make --profile mainnet test 
```

#### Running checks & lints

To run lints and checks, the following tasks are available:
- `check`, checks the format, clippy and ETH contracts.
- `check-contracts`, runs yarn lints on the ETH contracts.
- `check-fmt`, checks the workspace Rust format only.
- `check-clippy`, checks the Rust workspace with clippy only.

For example the following command will run the checks. `profile` is not required
here:
```
cargo make check
```

#### Cleanup

To clean up the workspace, the following tasks are available:
- `clean`, cleans all built binaries and ETH contracts.
- `clean-cargo`, cleans with cargo.
- `clean-contracts`, cleans the ETH contracts.
- `clean-bin`, cleans the binaries.

Additionally, there is also but not included in the `clean` task:
- `sweep`, sweeps the set amount of days in the ENV, default at 30 days.

For example, the following command will clean everything. `profile` is not 
required:
```
cargo make clean
```

## Deployment

### Downloading the latest EVM release

```sh
wget https://github.com/aurora-is-near/aurora-engine/releases/download/latest/mainnet-release.wasm
```

### Installing the Aurora CLI tool

```sh
npm install -g aurora-is-near/aurora-cli
```

### Deploying the EVM with the CLI

```sh
export NEAR_ENV=local
near delete aurora.test.near test.near  # if needed
near create-account aurora.test.near --master-account=test.near --initial-balance 1000000
aurora install --chain 1313161556 --owner test.near bin/mainnet-release.wasm
```

### Deploying the EVM without the CLI

```sh
export NEAR_ENV=local
near delete aurora.test.near test.near  # if needed
near create-account aurora.test.near --master-account=test.near --initial-balance 1000000
near deploy --account-id=aurora.test.near --wasm-file=bin/mainnet-release.wasm
aurora initialize --chain 1313161556 --owner test.near
```

## Usage

### Examining deployed EVM metadata

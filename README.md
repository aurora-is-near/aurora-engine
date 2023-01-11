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

| Network | Contract ID         | Chain ID   |
|---------|---------------------|------------|
| Mainnet | [`aurora`][Mainnet] | 1313161554 |
| Testnet | [`aurora`][Testnet] | 1313161555 |
| Local   | `aurora.test.near`  | 1313161556 |

[Mainnet]: https://explorer.near.org/accounts/aurora
[Testnet]: https://explorer.testnet.near.org/accounts/aurora

## Development

### Prerequisites

- Node.js (v14+)
- cargo-make

```sh
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
- `build-docker`, builds the `aurora-<profile>-test.wasm` in the `bin` folder using docker build environment. The purpose of this task is to produce reproducible binaries.

For example, the following will build the mainnet debug binary:
```sh
cargo make --profile mainnet build
```

#### Verifying binary hash

To verify that a deployed binary matches the source code, you may want build it reproducibly and then check that their hashes match. The motivation behind that is to prevent malicious code from being deployed.

Run these commands to produce the binary hash:
```sh
cargo make --profile <profile> build-docker
shasum -a 256 bin/aurora-<profile>.wasm
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

```sh
aurora get-version
aurora get-owner
aurora get-bridge-prover
aurora get-chain-id
```

### Deploying EVM contract bytecode

```sh
aurora deploy-code @contract.bytecode
```

```sh
aurora deploy-code 0x600060005560648060106000396000f360e060020a6000350480638ada066e146028578063d09de08a1460365780632baeceb714604d57005b5060005460005260206000f3005b5060016000540160005560005460005260206000f3005b5060016000540360005560005460005260206000f300
```

### Examining EVM contract state

```console
$ aurora encode-address test.near
0xCBdA96B3F2B8eb962f97AE50C3852CA976740e2B
```

```sh
aurora get-nonce 0xCBdA96B3F2B8eb962f97AE50C3852CA976740e2B
aurora get-balance 0xCBdA96B3F2B8eb962f97AE50C3852CA976740e2B
aurora get-code 0xFc481F4037887e10708552c0D7563Ec6858640d6
aurora get-storage-at 0xFc481F4037887e10708552c0D7563Ec6858640d6 0
```

### Calling an EVM contract read-only

```console
$ aurora encode-address test.near
0xCBdA96B3F2B8eb962f97AE50C3852CA976740e2B
```

```sh
aurora view --sender 0xCBdA96B3F2B8eb962f97AE50C3852CA976740e2B 0xFc481F4037887e10708552c0D7563Ec6858640d6 0x8ada066e  # getCounter()
aurora view --sender 0xCBdA96B3F2B8eb962f97AE50C3852CA976740e2B 0xFc481F4037887e10708552c0D7563Ec6858640d6 0xd09de08a  # increment()
aurora view --sender 0xCBdA96B3F2B8eb962f97AE50C3852CA976740e2B 0xFc481F4037887e10708552c0D7563Ec6858640d6 0x2baeceb7  # decrement()
```

### Calling an EVM contract mutatively

```sh
aurora call 0xFc481F4037887e10708552c0D7563Ec6858640d6 0xd09de08a  # increment()
aurora call 0xFc481F4037887e10708552c0D7563Ec6858640d6 0x2baeceb7  # decrement()
```

## Debugging

### Inspecting EVM storage state

```sh
near state aurora.test.near
aurora dump-storage
```

[`master`]:  https://github.com/aurora-is-near/aurora-engine/commits/master
[`develop`]: https://github.com/aurora-is-near/aurora-engine/commits/develop

## License
**aurora-engine** has multiple licenses:
* all crates except `engine-test` has **CCO-1.0** license
* `engine-test` has **GPL-v3** license

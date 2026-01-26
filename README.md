# Aurora Engine

[![Project license](https://img.shields.io/badge/License-Public%20Domain-blue.svg)](https://creativecommons.org/publicdomain/zero/1.0/)
[![Discord](https://img.shields.io/discord/490367152054992913?label=Discord)](https://discord.gg/jNjHYUF8vw)
[![Lints](https://github.com/aurora-is-near/aurora-engine/actions/workflows/lints.yml/badge.svg)](https://github.com/aurora-is-near/aurora-engine/actions/workflows/lints.yml)
[![Tests](https://github.com/aurora-is-near/aurora-engine/actions/workflows/tests.yml/badge.svg)](https://github.com/aurora-is-near/aurora-engine/actions/workflows/tests.yml)
[![Builds](https://github.com/aurora-is-near/aurora-engine/actions/workflows/builds.yml/badge.svg)](https://github.com/aurora-is-near/aurora-engine/actions/workflows/builds.yml)

Aurora Engine implements an Ethereum Virtual Machine (EVM) on the NEAR Protocol.
See [doc.aurora.dev](https://doc.aurora.dev/dev-reference/aurora-engine) for additional
documentation.

## Deployments

| Network | Contract ID         | Chain ID   |
|---------|---------------------|------------|
| Mainnet | [`aurora`][Mainnet] | 1313161554 |
| Testnet | [`aurora`][Testnet] | 1313161555 |
| Local   | `aurora.test.near`  | 1313161556 |

[Mainnet]: https://nearblocks.io/address/aurora
[Testnet]: https://testnet.nearblocks.io/address/aurora

## Development

### Prerequisites

- Node.js (v18+)
- cargo-make
- wasm-opt

```sh
cargo install --force cargo-make
```

### Prerequisites for Development

- Node.js (v18+)
- Docker
- cargo-make
- wasm-opt

### Prerequisite wasm-opt

For WebAssembly optimization we use `wasm-opt` from the [Binaryen toolchain for WebAssembly](https://github.com/WebAssembly/binaryen).

We recommend installing the release:
https://github.com/WebAssembly/binaryen/releases/tag/version_125

`wasm-opt` command should be available for the build process.

Verify version:

```bash
$ wasm-opt --version
wasm-opt version 125 (version_125)
```

Please be aware that you don't need to run `wasm-opt` explicitly, The `wasm-opt` runs automatically
when you run `cargo make build-*` and `cargo make test` commands.

## Development

### Branches

- [`master`] is the current stable branch.
  It must be ready, at all times, to be deployed on chain at a moment's notice.

- [`develop`] is our bleeding-edge development branch.
  In general, kindly target all pull requests to this branch.

#### Building the engine and contracts

There are several commands that can be used to build the binaries. The currently supported parameters
for the `task` field are listed below:

- `default`: does not need to be specified, runs `build`.
- `build`: builds included solidity contracts in the engine contract and engine smart contract itself and produces
 the file `aurora-engine.wasm` in the `bin` folder.
- `build-test`: builds the engine contract which is used in the integration tests and produces the 
 `aurora-engine-test.wasm` file in the `bin` folder.
- `build-contracts`: builds all the solidity contracts.
- `build-docker`: builds the `aurora-engine.wasm` in the `bin` folder using docker build environment.
  The purpose of this task is to produce reproducible binaries.

For example, the following will build the mainnet debug binary:

```sh
cargo make build
```

#### Verifying binary hash

To verify that a deployed binary matches the source code, you may want to build it reproducibly and then verify that
the SHA256 hash matches that of the deployed binary. The motivation behind this is to prevent malicious code from being
deployed.

Run these commands to produce the binary hash:

```sh
cargo make build-docker
shasum -a 256 bin/aurora-engine.wasm
```

#### Running unit & integration tests

To run tests, there are a few cargo make tasks we can run:
- `test-workspace`: tests only the cargo workspace.
- `test-contracts`: tests only the contracts.
- `test`: tests the whole cargo workspace, solidity contracts and runs modexp benchmarks.
- `test-flow`: tests the whole cargo workspace and solidity contracts.
- `bench-modexp`: runs modexp benchmarks.

For example, the following will test the whole workspace and solidity contracts:

```sh
cargo make test 
```

#### Running checks and lints

The following tasks are available to run lints and checks:

- `check`: checks the format, clippy and solidity contracts.
- `check-contracts` runs yarn lints on the solidity contracts.
- `check-fmt`: checks the workspace Rust format only.
- `clippy`: checks the Rust workspace with clippy only.

For example, the following command will run the checks. 
here:

```sh
cargo make check
```

#### Running WebAssembly optimization

In common cases, you don't need to run `wasm-opt` manually, because it's part of builds and tests.

But for development reasons only you can run:

- `wasm-opt` , runs WebAssembly optimization for pre-build wasm files.

For example, the following will run wasm-opt for pre-build mainnet binary:

```sh
cargo make wasm-opt 
```

#### Cleanup

The following tasks are available to clean up the workspace:

- `clean`: cleans all built binaries and solidity contracts.
- `clean-cargo`: cleans with cargo.
- `clean-contracts`: cleans the solidity contracts.
- `clean-bin`: cleans the binaries.

Additionally, there is also but not included in the `clean` task:

- `sweep`: cleans up unused build files for a period of time provided in the `time` argument. The `time` argument 
 is set in the ENV variable `SWEEP_DAYS`, default to 30 days.

For example, the following command will clean everything.

```sh
cargo make clean
```

[`master`]:  https://github.com/aurora-is-near/aurora-engine/commits/master
[`develop`]: https://github.com/aurora-is-near/aurora-engine/commits/develop

## License
**aurora-engine** has multiple licenses:
* All crates except `engine-test` have **CCO-1.0** license
* `engine-test` has **GPL-v3** license

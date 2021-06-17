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

Network | Contract ID         | Chain ID   | Status
------- | ------------------- | ---------- | ------
Mainnet | [`aurora`][Mainnet] | 1313161554 | 🚧
Testnet | [`aurora`][Testnet] | 1313161555 | 🚧
Betanet | [`aurora`][Betanet] | 1313161556 | 🚧
Local   | `aurora.test.near`  | 1313161556 | ✅

[Mainnet]: https://explorer.near.org/accounts/aurora
[Testnet]: https://explorer.testnet.near.org/accounts/aurora
[Betanet]: https://explorer.betanet.near.org/accounts/aurora

## Prerequisites

### Prerequisites for Building

- Rust nightly (2021-03-25) with the WebAssembly toolchain
- GNU Make (3.81+)

```sh
rustup install nightly-2021-03-25
rustup target add wasm32-unknown-unknown --toolchain nightly-2021-03-25
```

### Prerequisites for Development

- Node.js (v14+)

## Development

### Branches

- [`master`] is the current stable branch.
  It must be ready, anytime, to deployed on chain at a moment's notice.

- [`develop`] is our bleeding-edge development branch.
  In general, kindly target all pull requests to this branch.

### Building the EVM binary

```sh
make -B mainnet  # produces Mainnet build
make -B testnet  # produces Testnet build
make -B betanet  # produces Betanet build
make release  # produces release.wasm (300+ KiB)
make debug    # produces debug.wasm (1+ MiB), which includes symbols
```

### Running unit & integration tests

```sh
make check
```

## Deployment

### Downloading the latest EVM release

```sh
wget https://github.com/aurora-is-near/aurora-engine/releases/download/latest/release.wasm
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
aurora install --chain 1313161556 --owner test.near release.wasm
```

### Deploying the EVM without the CLI

```sh
export NEAR_ENV=local
near delete aurora.test.near test.near  # if needed
near create-account aurora.test.near --master-account=test.near --initial-balance 1000000
near deploy --account-id=aurora.test.near --wasm-file=release.wasm
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

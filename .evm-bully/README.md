# EVM-Bully testing

## Overview

EVM-Bully replays Ethereum-testnets transactions on aurora-engine.

For this purpose, it need's a couple of things:
1. [Synced testnets](https://github.com/aurora-is-near/evm-bully/blob/master/doc/server.md#synching-testnets)
2. Compiled [nearcore](https://github.com/near/nearcore)
3. Compiled [aurora-engine](https://github.com/aurora-is-near/aurora-engine/) contract
4. [Aurora-cli](https://github.com/aurora-is-near/aurora-cli) (for contract installation)

EVM-Bully goes from first to last transaction consequently and stops whenever any error happens.
Error message is then displayed, and breakpoint archive with a chain state is dumped.

`state.json` file keeps current breakpoints and locks dependency versions for CI consistency.
- Whenever state changes positively, it should be updated.
- Whenever state changes negatively, last aurora-engine changes should be reviewed.
- Version-locks should be updated regularly (we will have automation for that later).

## Current pipeline algorithm

1. Pull [aurora-cli](https://github.com/aurora-is-near/aurora-cli), run `npm install` on it
2. Build [aurora-engine](https://github.com/aurora-is-near/aurora-engine/) from current repo
3. Pull and build [evm-bully](https://github.com/aurora-is-near/evm-bully/)
4. Try download [nearcore](https://github.com/near/nearcore) for current platform, or otherwise pull it and build.
5. Run bully on each testnet (bully starts nearcore, installs contract and replays transactions)
6. Compare results with a `state.json` and inform on progress/downgrade

## How to run it locally?

### Prerequisites
1. Make sure your system have `npm`, `go`, `cargo`, `jq`, `make` and `git` installed.
2. [Sync testnets](https://github.com/aurora-is-near/evm-bully/blob/master/doc/server.md#synching-testnets)
3. Run [./evm-bully](https://github.com/aurora-is-near/evm-bully/) dumpdb [ -goerli | -ropsten | -rinkeby ] (to convert chains data to bully-readable format)
4. If you are not happy with this pipeline checkouting all repos, or you want to override some dependency, `cp config.sh.template config.sh` and edit it, following inner instructions.

### Running
1. Simply run `./run.sh [ goerli | ropsten | rinkeby ]`

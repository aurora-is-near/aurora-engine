# Benchmarks in the Aurora Engine

## What is being measured?

The primary quantity we are interested in measuring is the NEAR gas cost.
This limits how large of transactions we are able to process due to the [200 Tgas transaction limit](https://github.com/near/nearcore/blob/9a41274ddef3616ab195b24a207389c5ad5c7f5a/nearcore/res/genesis_config.json#L192) on NEAR.

As a secondary matter, we are interested in measuring the EVM gas used during a transaction since this is the way we expect many developers on our platform to think about the size of their transactions (since they come from the Ethereum ecosystem).
These measurements together can inform any correlation that exists between NEAR gas spent on Aurora and EVM gas spent on Ethereum.
So far we have seen that this correlation is not very strong, indicating that operations are costed very differently between our platform and Ethereum.

A final quantity of interest is the wall-clock time to execute a transaction.
This is less important for us as a smart contract on NEAR, but is reasonably important for the NEAR runtime itself.
At a high level, the NEAR runtime attempts to maintain the invariant that 1000 Tgas worth of computation can be completed in 1 second (regardless of which operations happen within this 1000 Tgas).
This arises from the [1000 Tgas gas limit](https://github.com/near/nearcore/blob/9a41274ddef3616ab195b24a207389c5ad5c7f5a/nearcore/res/genesis_config.json#L20) per block and the [1 second block time](https://github.com/near/nearcore/blob/9a41274ddef3616ab195b24a207389c5ad5c7f5a/nearcore/res/genesis_config.json#L238).
Moreover, the runtime wants the there to be a linear relationship between gas usage and time taken to complete the computation.
Obviously wall-clock time is not a stable metric as it varies according to the hardware and other details of the system running the test.
This means in reality the NEAR runtime measures gas costs by counting CPU instructions, with the assumption this has a rough correlation with wall-clock time.
For us, we measure wall-clock time because it is simpler and we do not need extreme precision the same way the runtime itself does.
The primary goal of these wall-clock measurements is simply to check the runtime's assumptions about the linear relationship between gas used and wall-clock time, and how much gas can be consumed in 1 second.

## How to do we measure these things?

NEAR gas is measured by [importing the NEAR runtime as a library](https://github.com/aurora-is-near/aurora-engine/blob/0fe4f0506866bd8813b270760864d22723925962/engine-tests/Cargo.toml#L34-L35), and executing our engine contract inside it.
There a [simple profiling structure](https://github.com/near/nearcore/blob/9a41274ddef3616ab195b24a207389c5ad5c7f5a/core/primitives-core/src/profile.rs#L49) that is returned by the runtime which breaks down how much NEAR gas was used by each host function which we also use.
We are working on a [more detailed profiling utility](https://github.com/birchmd/aurora-engine/blob/scoped-profiling/doc/profiling.md) that will allow breaking down costs spent on wasm computation within our contract since the majority of the gas is spent on wasm, not on host functions.

EVM gas is measured automatically as part of SputnikVM, so we get this for free.

Wall-clock time is measured using the [`criterion` rust benchmarking library](https://crates.io/crates/criterion).

## How are these tests/benchmarks run?

There are two types of benchmarks.

The first type are regression tests and simply running `make check` will execute them all (along with all the other tests we have).
Note: these use solidity contracts, so Docker, `yarn`, and `npm` are required dependencies in addition to the usual Rust tooling.
The regression tests check that no more than a given amount of NEAR gas is spent on a transaction.
This prevents us from introducing performance regressions as we continue to develop the engine.
If we suspect performance has actually improved we can print out the amount of gas used and change the bounds accordingly.
Some of these regression tests are discussed below.

The other type of benchmark is marked as `ignored` because they take too long to run to include in our usual CI.
These include the wall-clock time measurements discussed above.
They can be run by using the `--ignored` flag in `cargo test`.
They will print out amounts of gas used and time taken.
These values can be compared with previous runs to look for performance improvements / regressions.
Some of these benchmarks will also be included in the list below.

## Details of 5 specific benchmarks

Each benchmark below includes a description of contract what is being measured, how to run the benchmark
and some rationalization for how much better we would like to see that benchmark perform in the future.

### 1. Uniswap V3

This is a performance regression test.
It confirms it is possible to execute simple transactions involving the [Uniswap V3 protocol](https://docs.uniswap.org/protocol/reference/smart-contracts).
In particular the test creates a liquidity pool for a pair of tokens, adds liquidity and performs a swap.
[The test](https://github.com/aurora-is-near/aurora-engine/blob/a4c3cebbc5da0b14331601f2bff8047d276d2da0/engine-tests/src/tests/uniswap.rs#L24) can be run using the following command

```
make mainnet-test-build && cargo test --features mainnet-test uniswap
```

The adding liquidity operation consumes around 500k EVM gas, and around 165 NEAR Tgas.
With an EVM gas limit of 15 million on Ethereum. this means around 30 such operations could fit in one block.
30 such operations on Aurora would cost nearly 5000 Tgas, the equivalent of 5 blocks.
Therefore, for this benchmark we aim to have it cost 1/5th the amount of NEAR gas it does presently.

A wall-clock measurement using the uniswap contract also exists.
[That benchmark](https://github.com/aurora-is-near/aurora-engine/blob/a4c3cebbc5da0b14331601f2bff8047d276d2da0/engine-tests/src/benches/mod.rs#L42) can be run using the following command

```
make mainnet-test-build && cargo test --features mainnet-test uniswap -- --ignored --nocapture
```

### 2. 1inch liquidity protocol

This is a performance regression test.
It confirms it is possible to execute simple transactions involving the [1inch liquidity protocol](https://github.com/1inch/liquidity-protocol).
[The test](https://github.com/aurora-is-near/aurora-engine/blob/0fe4f0506866bd8813b270760864d22723925962/engine-tests/src/tests/one_inch.rs#L17) can be run using the following command

```
make mainnet-test-build && cargo test --features mainnet-test 1inch
```

The operation depositing funds into the 1inch liquidity pool consumes around 300k EVM gas, and around 120 NEAR Tgas.
With an EVM gas limit of 15 million on Ethereum. this means around 50 such operations could fit in one block.
50 such operations on Aurora would cost 6000 Tgas, the equivalent of 6 blocks.
Therefore, for this benchmark we aim to have it cost 1/6th the amount of NEAR gas it does presently.

### 3. NFT paginated view call

This is an `ignored` benchmark test.
It takes a very long time to run, so we do not include it in CI.
This test was inspired by a partner who was hitting the gas limit in view calls.
The purpose of the test was to see how high we would need to set the gas limit to enable their use case.
[The test](https://github.com/aurora-is-near/aurora-engine/blob/0fe4f0506866bd8813b270760864d22723925962/engine-tests/src/benches/mod.rs#L25) measures how much gas is needed to display [different numbers of NFTs](https://github.com/aurora-is-near/aurora-engine/blob/0fe4f0506866bd8813b270760864d22723925962/engine-tests/src/benches/mod.rs#L28) (per page) of [different sizes](https://github.com/aurora-is-near/aurora-engine/blob/0fe4f0506866bd8813b270760864d22723925962/engine-tests/src/benches/mod.rs#L27) (that is the size of the metadata in bytes).
Results from the last run of this test can be seen [here](https://github.com/aurora-is-near/aurora-engine/issues/199#issuecomment-906747906).
The benchmark can be run using the following command

```
make mainnet-test-build && cargo test --features mainnet-test nft_pagination -- --ignored --nocapture
```

Based on this measurement, we estimate it would take 2000 Tgas to complete this operation with reasonable values of NFT size and number of NFTs per page.
Since this operation is meant to complete in a single transaction, it must fit in 200 Tgas.
Therefore, for this benchmark we aim to have it cost 1/10th the amount of NEAR gas it does presently.

### 4. Deploying the largest possible contract

This is a performance regression test.
It checks we are able to deploy all possible EVM smart contracts (without any initialization logic) by showing the largest allowed (in terms of number of bytes) is able to be deployed within the gas limit.
[The test](https://github.com/aurora-is-near/aurora-engine/blob/a4c3cebbc5da0b14331601f2bff8047d276d2da0/engine-tests/src/tests/sanity.rs#L45) can be run using the following command

```
make mainnet-test-build && cargo test --features mainnet-test deploy_largest_contract
```

This operation costs around 5 million EVM gas, meaning 3 such operations could happen in one block.
At just 43 Tgas, we can already fit many more than 3 such operation into a single block on Aurora.
Therefore, we do not need improvement on this benchmark.
It is included here in case NEAR ever decided to raise the gas price on storage operations.

### 5. `ecpair` precompile

This single operation consumes [almost 2000 Tgas by itself](https://github.com/near/nearcore/issues/4787#issuecomment-920031553)!
In order to be fully Ethereum compatible we must be able to execute the [`ecpair` precompile](https://eips.ethereum.org/EIPS/eip-197).
It may be the case that this cannot be done efficiently enough in wasm and it will need to become a host function in the NEAR runtime instead (this was done for [`ecrecover` for example](https://github.com/near/nearcore/pull/4380)).
[The test](https://github.com/aurora-is-near/aurora-engine/blob/a4c3cebbc5da0b14331601f2bff8047d276d2da0/engine-tests/src/tests/standard_precompiles.rs#L24) is listed as `ignored` currently because the amount of gas it uses is too large.
It can be run using the following command

```
make mainnet-test-build && cargo test --features mainnet-test ecpair -- --ignored --nocapture
```

On Ethereum this operation would cost around 135k EVM gas, and thus could be repeated over 100 times in a single Ethereum block.
On aurora, repeating this operation 100 times would cost 200_000 Tgas, the equivalent of 200 blocks.
Therefore, for this benchmark we aim to have it cost 1/200th the amount of NEAR gas it does presently.

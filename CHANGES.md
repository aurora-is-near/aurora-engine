# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [2.2.0] - 2021-11-09

### Added

- Depositing ETH from Ethereum to Aurora now allows an `0x` prefix on the recipeint address by [@joshuajbouw]. ([#337](https://github.com/aurora-is-near/aurora-engine/pull/337))

## [2.1.0] - 2021-11-04

### Fixed

- Bug in `ft_transfer_call` and `ft_resolve_transfer` by  [@birchmd] and [@mrLSD]. ([#326](https://github.com/aurora-is-near/aurora-engine/pull/326) [#330](https://github.com/aurora-is-near/aurora-engine/pull/330))
- Incorrect gas cost on ripemd precompile by [@joshuajbouw]. ([#329](https://github.com/aurora-is-near/aurora-engine/pull/329))

## [2.0.2] - 2021-11-01

### Added

- Logging number of storage writes by [@birchmd]. ([#322](https://github.com/aurora-is-near/aurora-engine/pull/322))

### Fixed

- Show full address in logging transaction sender on `submit` by [@birchmd]. ([#321](https://github.com/aurora-is-near/aurora-engine/pull/321))

## [2.0.1] - 2021-11-01

### Added

- Added logging of public keys during `submit` calls by [@joshuajbouw]. ([#319](https://github.com/aurora-is-near/aurora-engine/pull/319))

## [2.0.0] - 2021-10-27

### Added

- Public method for computing Aurora blockhash at a given hight by [@birchmd]. ([#303](https://github.com/aurora-is-near/aurora-engine/pull/303))

### Changed

- EVM logs returned in `SubmitResult` include the address the log originates from by [@birchmd]. ([#299](https://github.com/aurora-is-near/aurora-engine/pull/299))
  - Note: this is a breaking change in the `SubmitResult` binary format.

### Fixed

- Precompile bug fixes by [@birchmd]. ([#305](https://github.com/aurora-is-near/aurora-engine/pull/305), [#306](https://github.com/aurora-is-near/aurora-engine/pull/306))
- Update to latest `rust-blockchain/evm` version (fixes bug in `JUMPI`) EVM opcode by [@birchmd]. ([#316](https://github.com/aurora-is-near/aurora-engine/pull/316))

## [1.7.0] - 2021-10-13

### Changes

- Add EVM events for exit precompiles by [@birchmd]

## [1.6.4] - 2021-09-29

### Changes

- Fix JSON formatting in `ft_metadata` method by [@birchmd].
- Fix a bug in `block.timestamp` (units should be seconds) by [@birchmd].

## [1.6.3] - 2021-09-14

### Changes

- Revert the ERC-20 admin address changes for the time being by [@joshuajbouw].

## [1.6.2] - 2021-09-13

### Changes

- Change the ERC-20 admin address to have a dedicated account by [@sept-en].
- Fix precompile promises that were broken in rust-blockchain/evm by
  [@joshuajbouw] and [@birchmd].
- Fix the return format of `ft_balance_of` by [@joshuajbouw].

### Removed

- Remove Testnet balancing `balance_evm_and_nep141` by [@birchmd].

## [1.6.1] - 2021-08-23

### Breaking changes

- Update the `view` call to correctly return the Borsh serialization of
  `TransactionStatus`. Previously, it returned a string with the result of
  the transaction by name.

- Change the `ft_balance_of` result as previously it returned a non-JSON
  string value `0`. This has been fixed to return `"0"`.

## [1.6.0] - 2021-08-13

### Breaking changes

- Change the transaction status of `submit` as running out of gas,
  funds, or being out-of-the-offset are not fatal errors but failed
  executions.

The `submit` call altered the `SubmitResult` object to the following format:

```rust
enum TransactionStatus {
    Succeed(Vec<u8>),
    Revert(Vec<u8>),
    OutOfGas,
    OutOfFund,
    OutOfOffset,
    CallTooDeep,
}

struct ResultLog {
    topics: Vec<[u8; 32]>,
    data: Vec<u8>,
}

struct SubmitResult {
    status: TransactionStatus, // above
    gas_used: u64,
    logs: Vec<ResultLog>,
}
```

## [1.5.0] - 2021-07-30

## [1.4.3] - 2021-07-08

## [1.4.2] - 2021-06-25

## [1.4.1] - 2021-06-23

## [1.4.0] - 2021-06-18

## [1.3.0] - 2021-06-17

## [1.2.0] - 2021-06-05

## [1.1.0] - 2021-05-28

## [1.0.0] - 2021-05-12

[Unreleased]: https://github.com/aurora-is-near/aurora-engine/compare/2.2.0...master
[2.2.0]: https://github.com/aurora-is-near/aurora-engine/compare/2.1.0...2.2.0
[2.1.0]: https://github.com/aurora-is-near/aurora-engine/compare/2.0.2...2.1.0
[2.0.2]: https://github.com/aurora-is-near/aurora-engine/compare/2.0.1...2.0.2
[2.0.1]: https://github.com/aurora-is-near/aurora-engine/compare/2.0.0...2.0.1
[2.0.0]: https://github.com/aurora-is-near/aurora-engine/compare/1.7.0...2.0.0
[1.7.0]: https://github.com/aurora-is-near/aurora-engine/compare/1.6.4...1.7.0
[1.6.4]: https://github.com/aurora-is-near/aurora-engine/compare/1.6.3...1.6.4
[1.6.3]: https://github.com/aurora-is-near/aurora-engine/compare/1.6.2...1.6.3
[1.6.2]: https://github.com/aurora-is-near/aurora-engine/compare/1.6.1...1.6.2
[1.6.1]: https://github.com/aurora-is-near/aurora-engine/compare/1.6.0...1.6.1
[1.6.0]: https://github.com/aurora-is-near/aurora-engine/compare/1.5.0...1.6.0
[1.5.0]: https://github.com/aurora-is-near/aurora-engine/compare/1.4.3...1.5.0
[1.4.3]: https://github.com/aurora-is-near/aurora-engine/compare/1.4.2...1.4.3
[1.4.2]: https://github.com/aurora-is-near/aurora-engine/compare/1.4.1...1.4.2
[1.4.1]: https://github.com/aurora-is-near/aurora-engine/compare/1.4.0...1.4.1
[1.4.0]: https://github.com/aurora-is-near/aurora-engine/compare/1.3.0...1.4.0
[1.3.0]: https://github.com/aurora-is-near/aurora-engine/compare/1.2.0...1.3.0
[1.2.0]: https://github.com/aurora-is-near/aurora-engine/compare/1.1.0...1.2.0
[1.1.0]: https://github.com/aurora-is-near/aurora-engine/compare/1.0.0...1.1.0
[1.0.0]: https://github.com/aurora-is-near/aurora-engine/tree/1.0.0

[@mrLSD]: https://github.com/mrLSD
[@birchmd]: https://github.com/birchmd
[@joshuajbouw]: https://github.com/joshuajbouw
[@sept-en]: https://github.com/sept-en

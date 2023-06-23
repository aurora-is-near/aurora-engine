# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [2.9.2] 2023-06-22

### Fixes

- Use ibig implemenation of modexp by [@birchmd]. ([#778])

[#778]: https://github.com/aurora-is-near/aurora-engine/pull/778

## [2.9.1] 2023-05-11

### Changes

- Removed unused state variable `bridge_prover_id` by [@birchmd]. ([#749])
- `modexp` has been improved to be greatly faster than before by [@birchmd]. ([#757])

### Fixes

- Fixed an issue where the owner could call `new` multiple times by [@lempire123]. ([#733])
- Fixed an issue with `deploy_upgrade` where the upgrade index isn't cleared by [@lempire123]. ([#741])

[#749]: https://github.com/aurora-is-near/aurora-engine/pull/749
[#757]: https://github.com/aurora-is-near/aurora-engine/pull/757
[#733]: https://github.com/aurora-is-near/aurora-engine/pull/733
[#741]: https://github.com/aurora-is-near/aurora-engine/pull/741

## [2.9.0] 2023-04-05

### Added

- Enabled XCC for mainnet release by [@birchmd]. ([#694])
- Added `set_owner` contract method which sets the owner of the contract by [@hskang9]. ([#690])
- New variant of submit function `submit_with_args` which accepts additional arguments along with the transaction such as the max gas price a user is ready to pay by [@aleksuss]. ([#696])
- Added the ability to create and fund XCC sub-accounts from external NEAR accounts by [@birchmd]. ([#735])

### Changes

- Replaced `rjson` with `serde_json` by [@aleksuss]. ([#677])
- Changed owner intended contract methods to now require owner or the contract itself by [@hskang9]. ([#676])

### Fixes

- Fixed nonce incorrectly being incremented on an out of fund failure by [@joshuajbouw]. ([#671])
- Fixed a check in promise results before executing cross contract calls (XCC) callbacks by [@birchmd]. ([#693])
- Fixed a reachable panic in `receive_erc20_tokens` by [@0x3bfc]. ([#709])
- Fixed a lack of minimum size checks when instantiating a new `EthGas` object by [@lempire123]. ([#722])
- Fixed a lack of division by 0 checks in `EthGas::Div()` by [@lempire123]. ([#718])
- Fixed the validation of the return of `exports:storage_remove` by [@0x3bfc]. ([#712])
- Fixed missing account validations of NEAR account IDs by [@0x3bfc]. ([#703])
- Fixed a reachable panic in the `exitToNear` and `exitToEthereum` precompiles if the input amount is greater than 1^128 when cast from a `U256` to `u128` by [@0x3bfc]. ([#681])
- Fixed a reachable panic in `modExp` due to arithmetic overflow by [@0x3bfc]. ([#688])
- Fixed the ability attaching values to Aurora specific precompiles, this no longer is possible, by [@0x3bfc]. ([#714])
- Fixed a return error if an ecrecover signature length is not exactly 65 by [@0x3bfc]. ([#717])
- Fixed size checks on input array passed to `exitToNear` and `exitToEthereum` precompiles by [@0x3bfc]. ([#684])
- Fixed missing gas costs in `exitToNear` and `exitToEthereum` precompiles by [@lempire123]. ([#687])
- Fixed a reachable panic due to out of memory in the `modExp` precompile by [@0x3bfc]. ([#689])
- Fixed an assurance that the `sender_id` has a balance greater than the amount in `ft_transfer_call` by [@0x3bfc]. ([#708])
- Fixed returning `0x` when a length cannot be cast as `usize` instead of returning an error in the `modExp` precompile by [@birchmd]. ([#737])
- Miscellaneous minor fixes by [@0x3bfc]. ([#738])

[#671]: https://github.com/aurora-is-near/aurora-engine/pull/671
[#677]: https://github.com/aurora-is-near/aurora-engine/pull/677
[#693]: https://github.com/aurora-is-near/aurora-engine/pull/693
[#694]: https://github.com/aurora-is-near/aurora-engine/pull/694
[#690]: https://github.com/aurora-is-near/aurora-engine/pull/690
[#676]: https://github.com/aurora-is-near/aurora-engine/pull/676
[#709]: https://github.com/aurora-is-near/aurora-engine/pull/709
[#722]: https://github.com/aurora-is-near/aurora-engine/pull/722
[#718]: https://github.com/aurora-is-near/aurora-engine/pull/718
[#696]: https://github.com/aurora-is-near/aurora-engine/pull/696
[#712]: https://github.com/aurora-is-near/aurora-engine/pull/712
[#703]: https://github.com/aurora-is-near/aurora-engine/pull/703
[#681]: https://github.com/aurora-is-near/aurora-engine/pull/681
[#688]: https://github.com/aurora-is-near/aurora-engine/pull/688
[#714]: https://github.com/aurora-is-near/aurora-engine/pull/714
[#717]: https://github.com/aurora-is-near/aurora-engine/pull/717
[#684]: https://github.com/aurora-is-near/aurora-engine/pull/684
[#687]: https://github.com/aurora-is-near/aurora-engine/pull/687
[#689]: https://github.com/aurora-is-near/aurora-engine/pull/689
[#708]: https://github.com/aurora-is-near/aurora-engine/pull/708
[#737]: https://github.com/aurora-is-near/aurora-engine/pull/737
[#735]: https://github.com/aurora-is-near/aurora-engine/pull/735
[#738]: https://github.com/aurora-is-near/aurora-engine/pull/738

## [2.8.1] 2022-12-07

- Updated SputnikVM to v0.37.2 by [@birchmd]. ([#645])

## [2.8.1] 2022-12-07

### Changes

- Performance improvement (approximately 5% reduction in NEAR gas usage) by [@birchmd]. ([#645])

### Fixes

- Tracing bug fix by [@birchmd]. ([#646])

[#645]: https://github.com/aurora-is-near/aurora-engine/pull/645
[#646]: https://github.com/aurora-is-near/aurora-engine/pull/646

## [2.8.0] 2022-11-15

### Added

- New functions `pause_precompiles` and `resume_precompiles` to allow pausing/unpausing the exit precompiles on Aurora by [@RomanHodulak]. ([#588])
- Reproducible build in Docker by [@RomanHodulak]. ([#633])

### Fixes

- Update to latest SputnikVM by [@birchmd] (fixes some security issues including a potential call stack overflow and incorrect `is_static` indicator in exit precompiles). ([#628])
- Minor fixes for the XCC functionality by [@birchmd]. ([#610] [#616] [#622])
- Fix bn256 regression by [@joshuajbouw]. ([#637])

[#588]: https://github.com/aurora-is-near/aurora-engine/pull/588
[#610]: https://github.com/aurora-is-near/aurora-engine/pull/610
[#616]: https://github.com/aurora-is-near/aurora-engine/pull/616
[#622]: https://github.com/aurora-is-near/aurora-engine/pull/622
[#628]: https://github.com/aurora-is-near/aurora-engine/pull/628
[#633]: https://github.com/aurora-is-near/aurora-engine/pull/633
[#637]: https://github.com/aurora-is-near/aurora-engine/pull/637

## [2.7.0] 2022-08-19

### Added
- Get promise results precompile at address on `testnet` `0x0a3540f79be10ef14890e87c1a0040a68cc6af71` by [@birchmd]. ([#575])
- Cross-contract calls to NEAR contracts are now available for `testnet` by [@birchmd] and [@mfornet]. ([#560])

### Changes
- Use NEAR host functions for alt bn256 precompile by [@birchmd], [@joshuajbouw], and [@RomanHodulak]. ([#540])

### Fixes
- Fixed an issue where a transaction can panic on an empty input by [@birchmd]. ([#573])
- Return the correct value while using the `get_bridge_prover` method by [@birchmd]. ([#581])

[#575]: https://github.com/aurora-is-near/aurora-engine/pull/575
[#560]: https://github.com/aurora-is-near/aurora-engine/pull/560
[#540]: https://github.com/aurora-is-near/aurora-engine/pull/540
[#573]: https://github.com/aurora-is-near/aurora-engine/pull/573
[#581]: https://github.com/aurora-is-near/aurora-engine/pull/581

## [2.6.1] 2022-06-23

### Fixes

- Fixed an issue with accounting being problematic with the total supply of ETH on Aurora as it could artificially deplete by [@birchmd]. ([#536])
- Fixed the possibility of forging receipts to allow for withdrawals on the Rainbow Bridge by [@birchmd], [@mfornet], [@sept-en] and [@joshuajbouw]. Written by [@birchmd].
- Fixed the ability the steal funds from those by setting a fee when receiving NEP-141 as ERC-20 by [@birchmd], [@mfornet], and [@joshuajbouw]. Written by [@joshuajbouw].

[#536]: https://github.com/aurora-is-near/aurora-engine/pull/536

## [2.6.0] 2022-06-08

### Added

- A precompile at the address `0x536822d27de53629ef1f84c60555689e9488609f` was created to expose the prepaid gas from the NEAR host function by [@birchmd]. ([#479])

### Changes

- A better implementation of caching was added to reduce the overall gas costs of storage reads resulting in roughly a 15% - 18% reduction of gas costs by [@birchmd]. ([#488])

### Fixes

- If the `v` byte of secp256k1 is incorrect, it now returns correctly an empty vector by [@RomanHodulak]. ([#513])
- Original ETH transactions which do not contain a Chain ID are allowed again to allow for use of [EIP-1820] by [@joshuajbouw]. ([#520])
- Ecrecover didn't reject `r`, `s` values larger than curve order by [@RomanHodulak]. ([#515])
- The predecessor account ID was failing in the `view` method by [@birchmd]. ([#477])
- Ecrecover was incorrectly setting the NEAR ecrecover malleability flag by [@birchmd] and [@joshuajbouw]. ([#474])

[EIP-1820]: https://eips.ethereum.org/EIPS/eip-1820
[#520]: https://github.com/aurora-is-near/aurora-engine/pull/520
[#515]: https://github.com/aurora-is-near/aurora-engine/pull/515
[#513]: https://github.com/aurora-is-near/aurora-engine/pull/513
[#488]: https://github.com/aurora-is-near/aurora-engine/pull/488
[#479]: https://github.com/aurora-is-near/aurora-engine/pull/479
[#477]: https://github.com/aurora-is-near/aurora-engine/pull/477
[#474]: https://github.com/aurora-is-near/aurora-engine/pull/474

## [2.5.3] 2022-04-27

### Fixes

- Fixed inflation vulnerability relating to ExitToNear and ExitToEthereum by [@birchmd], [@mfornet], and [@joshuajbouw]. Written by [@birchmd].

## [2.5.2] 2022-03-22

### Removed

- New Aurora-only precompiles removed since they do not work in NEAR view calls. This will need to be fixed and they will be re-added to a future release.

## [2.5.1] - 2022-03-16

### Fixes

- Fix for bug in checking address exists introduced in the v2.5.0 release by [@birchmd]. ([#469])

### Added

- New Aurora-only precompiles for checking the current and predecessor NEAR account IDs by [@birchmd]. ([#462])

[#462]: https://github.com/aurora-is-near/aurora-engine/pull/462
[#469]: https://github.com/aurora-is-near/aurora-engine/pull/469

## [2.5.0] - 2022-03-09

### Changes

- Performance improvement by [@birchmd] and [@olonho]: ([#455]) ([#456])

### Fixes

- Bug fix for the behaviour of transactions to the zero address by [@birchmd]. ([#458])

[#455]: https://github.com/aurora-is-near/aurora-engine/pull/455
[#456]: https://github.com/aurora-is-near/aurora-engine/pull/456
[#458]: https://github.com/aurora-is-near/aurora-engine/pull/458

## [2.4.0] - 2022-02-16

### Changes

- Performance improvements by [@birchmd] and [@matklad]; the engine should now consume much less NEAR gas: ([#427]) ([#438]) ([#439]) ([#445]) ([#446])
- Security improvment: only Engine contract owner can use the `deploy_upgrade` method by [@birchmd]. ([#410])
- Bug fix: Engine now returns the error message in the case of a revert during an EVM contract deploy, previously it would always return an address (even when the deploy failed) by [@birchmd]. ([#424])
- Security improvment: Engine will no longer accept EVM transactions without a chain ID as part of their signature by [@birchmd]. This should have no impact on users as all modern Ethereum tooling includes the chain ID. ([#432])
- Improvements to code quality by [@mrLSD]: ([#386]) ([#387])
- Improvements and additions to internal tests and benchmarks by [@birchmd]: ([#408]) ([#415]) ([#429])

[#386]: https://github.com/aurora-is-near/aurora-engine/pull/386
[#387]: https://github.com/aurora-is-near/aurora-engine/pull/387
[#408]: https://github.com/aurora-is-near/aurora-engine/pull/408
[#410]: https://github.com/aurora-is-near/aurora-engine/pull/410
[#415]: https://github.com/aurora-is-near/aurora-engine/pull/415
[#424]: https://github.com/aurora-is-near/aurora-engine/pull/424
[#427]: https://github.com/aurora-is-near/aurora-engine/pull/427
[#429]: https://github.com/aurora-is-near/aurora-engine/pull/429
[#432]: https://github.com/aurora-is-near/aurora-engine/pull/432
[#438]: https://github.com/aurora-is-near/aurora-engine/pull/438
[#439]: https://github.com/aurora-is-near/aurora-engine/pull/439
[#445]: https://github.com/aurora-is-near/aurora-engine/pull/445
[#446]: https://github.com/aurora-is-near/aurora-engine/pull/446

## [2.3.0] - 2021-12-10

### Added

- A precompile which exposes NEAR's random number generator was added by [@mfornet] as requested by
  [@birchmd]. ([#368] [#297])
- London hard fork support was added by [@birchmd]. ([#244])

### Changes

- The gas limit for `deposit` and `ft_on_transfer` were changed as they were not attaching enough
  gas, as changed by [@mrLSD]. ([#389])

### Fixes

- There was an issue with the original storage not actually being stored. Unfortunately, previous
  transactions can't be updated with this change. This has been fixed by [@birchmd]. ([#390])
- Call arguments were intended to have a value attached to them to make it equivalent to an ETH
  call. This was fixed in a backwards compatible manner by [@andrcmdr], as reported by [@birchmd].
  ([#351] [#309])

### Removed

- Betanet support was dropped and will no longer be supported by [@joshuajbouw]. ([#388])

[#390]: https://github.com/aurora-is-near/aurora-engine/pull/390
[#389]: https://github.com/aurora-is-near/aurora-engine/pull/389
[#388]: https://github.com/aurora-is-near/aurora-engine/pull/388
[#368]: https://github.com/aurora-is-near/aurora-engine/pull/368
[#351]: https://github.com/aurora-is-near/aurora-engine/pull/351
[#311]: https://github.com/aurora-is-near/aurora-engine/pull/311
[#309]: https://github.com/aurora-is-near/aurora-engine/issues/309
[#297]: https://github.com/aurora-is-near/aurora-engine/issues/297
[#244]: https://github.com/aurora-is-near/aurora-engine/pull/244

## [2.2.0] - 2021-11-09

### Added

- Depositing ETH from Ethereum to Aurora now allows an `0x` prefix on the recipient address by [@joshuajbouw]. ([#337](https://github.com/aurora-is-near/aurora-engine/pull/337))

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

[Unreleased]: https://github.com/aurora-is-near/aurora-engine/compare/2.9.2...develop
[2.9.2]: https://github.com/aurora-is-near/aurora-engine/compare/2.9.1...2.9.2
[2.9.1]: https://github.com/aurora-is-near/aurora-engine/compare/2.9.0...2.9.1
[2.9.0]: https://github.com/aurora-is-near/aurora-engine/compare/2.8.0...2.9.0 
[2.8.0]: https://github.com/aurora-is-near/aurora-engine/compare/2.7.0...2.8.0
[2.7.0]: https://github.com/aurora-is-near/aurora-engine/compare/2.6.1...2.7.0
[2.6.1]: https://github.com/aurora-is-near/aurora-engine/compare/2.6.0...2.6.1
[2.6.0]: https://github.com/aurora-is-near/aurora-engine/compare/2.5.3...2.6.0
[2.5.3]: https://github.com/aurora-is-near/aurora-engine/compare/2.5.2...2.5.3
[2.5.2]: https://github.com/aurora-is-near/aurora-engine/compare/2.5.1...2.5.2
[2.5.1]: https://github.com/aurora-is-near/aurora-engine/compare/2.5.0...2.5.1
[2.5.0]: https://github.com/aurora-is-near/aurora-engine/compare/2.4.0...2.5.0
[2.4.0]: https://github.com/aurora-is-near/aurora-engine/compare/2.3.0...2.4.0
[2.3.0]: https://github.com/aurora-is-near/aurora-engine/compare/2.2.0...2.3.0
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

[@0x3bfc]: https://github.com/0x3bfc
[@aleksuss]: https://github.com/aleksuss
[@andrcmdr]: https://github.com/andrcmdr
[@birchmd]: https://github.com/birchmd
[@hskang9]: https://github.com/hskang9
[@joshuajbouw]: https://github.com/joshuajbouw
[@mfornet]: https://github.com/mfornet
[@mrLSD]: https://github.com/mrLSD
[@RomanHodulak]: https://github.com/RomanHodulak
[@sept-en]: https://github.com/sept-en
[@matklad]: https://github.com/matklad
[@olonho]: https://github.com/olonho

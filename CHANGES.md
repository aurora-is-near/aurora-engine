# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [3.9.1] 2025-07-09

- Improve security of the `exit_to_near` precompile by [@aleksuss]. ([#1024])

[#1024]: https://github.com/aurora-is-near/aurora-engine/pull/1024

## [3.9.0] 2025-04-07

### Additions

- Added a new transaction `set_erc20_fallback_address` by [@aleksuss]. ([#1009])

### Changes

- The `ecrecover` implementation was moved to the `aurora-engine-sdk` by [@aleksuss]. ([#996])
- Updated the catalog info by [@diegofigs]. ([#1002]) 
- Usage of the `aurora-evm` crate from `crates.io` by [@mrLSD]. ([#1003])
- The white lists don't require the fixed gas per transaction (silo mode) by [@aleksuss]. ([#1005])
- Made the API compatible with the latest version of the eth connector by [@aleksuss]. ([#1006])

### Fixes

- Fixed the incorrect amount of gas provided to promises to eth connector by [@aleksuss]. ([#1000])

[#996]: https://github.com/aurora-is-near/aurora-engine/pull/996
[#1000]: https://github.com/aurora-is-near/aurora-engine/pull/1000
[#1002]: https://github.com/aurora-is-near/aurora-engine/pull/1002
[#1003]: https://github.com/aurora-is-near/aurora-engine/pull/1003
[#1005]: https://github.com/aurora-is-near/aurora-engine/pull/1005
[#1006]: https://github.com/aurora-is-near/aurora-engine/pull/1006
[#1009]: https://github.com/aurora-is-near/aurora-engine/pull/1009

## [3.8.0] 2025-02-05

### Changes

- Bumped the SputnikVM up to 0.46.1-aurora by [@mrLSD]. ([#966])
- The nightly rust compiler has been replaced with stable by [@aleksuss]. ([#977])
- Added changes regarding bumping the `primitive-types` crate by [@mrLSD]. ([#982])
- The code coverage by clippy has been extended by [@aleksuss]. ([#984])
- The code is changed by the clippy suggestions from the toolchain 1.84.0 by [@mrLSD]. ([#986])
- The precompile `exitToNear` now is compatible with the OMNI bridge by [@aleksuss]. ([#992])

### Fixes

- Added optimisations to the `AccountId` creation methods by [@aleksuss]. ([#985])
- The `README.md` has been actualized by [@aleksuss]. ([#978])
- Modified CI scripts by [@aleksuss]. ([#969], [#973], [#975], [#976], [#981])
- Fixed the vulnerability in the `remove_relayer_key` transaction by [@aleksuss]. ([#972]) 

[#966]: https://github.com/aurora-is-near/aurora-engine/pull/966
[#969]: https://github.com/aurora-is-near/aurora-engine/pull/969
[#972]: https://github.com/aurora-is-near/aurora-engine/pull/972
[#973]: https://github.com/aurora-is-near/aurora-engine/pull/973
[#975]: https://github.com/aurora-is-near/aurora-engine/pull/975
[#976]: https://github.com/aurora-is-near/aurora-engine/pull/976
[#977]: https://github.com/aurora-is-near/aurora-engine/pull/977
[#978]: https://github.com/aurora-is-near/aurora-engine/pull/978
[#981]: https://github.com/aurora-is-near/aurora-engine/pull/981
[#982]: https://github.com/aurora-is-near/aurora-engine/pull/982
[#984]: https://github.com/aurora-is-near/aurora-engine/pull/984
[#985]: https://github.com/aurora-is-near/aurora-engine/pull/985
[#986]: https://github.com/aurora-is-near/aurora-engine/pull/986
[#992]: https://github.com/aurora-is-near/aurora-engine/pull/992

## [3.7.0] 2024-10-09

### Additions

- Added support of CANCUN hardfork by [@mrLSD]. ([#926])
- Added support of EIP-3607 by [@mrLSD]. ([#930])
- Removed restrictions from funding XCC sub-accounts by [@birchmd]. ([#931])

### Changes

- Made some EVM gas costs optimisations by [@mrLSD]. ([#934])
- Refactored the gas charge logic form EVM exit reasons by [@mrLSD]. ([#935])
- Updated some dependencies and rust-toolchain by [@mrLSD]. ([#936])
- Removed unused `bytes_to_hex` function by [@dwiekawki]. ([#942])
- Added building of actual version of the `near-sandbox` in the scheduled CI job by [@aleksuss] ([#950])

### Fixes

- Removed duplicated `test` task in the `README.md` by [@dwiekawki]. ([#943])
- Fixed some typos in the `README.md` and `Cargo.toml` by [@DemoYeti]. ([#945]) ([#946])
- Fixed exceeded prepaid gas error in the `mirror_erc20_token` transaction by [@aleksuss] ([#951])
- Modified `hardhat.config.js` to support contract verification by [@spilin] ([#958])

[#926]: https://github.com/aurora-is-near/aurora-engine/pull/926
[#930]: https://github.com/aurora-is-near/aurora-engine/pull/930
[#931]: https://github.com/aurora-is-near/aurora-engine/pull/931
[#934]: https://github.com/aurora-is-near/aurora-engine/pull/934
[#935]: https://github.com/aurora-is-near/aurora-engine/pull/935
[#936]: https://github.com/aurora-is-near/aurora-engine/pull/936
[#942]: https://github.com/aurora-is-near/aurora-engine/pull/942
[#943]: https://github.com/aurora-is-near/aurora-engine/pull/943
[#945]: https://github.com/aurora-is-near/aurora-engine/pull/945
[#946]: https://github.com/aurora-is-near/aurora-engine/pull/946
[#950]: https://github.com/aurora-is-near/aurora-engine/pull/950
[#951]: https://github.com/aurora-is-near/aurora-engine/pull/951
[#958]: https://github.com/aurora-is-near/aurora-engine/pull/958

## [3.6.4] 2024-07-22

### Additions

- Added a possibility to provide amount of gas for the `state_migration` callback in the `upgrade`
  transaction by [@aleksuss]. ([#937])

[#937]: https://github.com/aurora-is-near/aurora-engine/pull/937

## [3.6.3] 2024-04-16

### Additions

- Added a possibility to pause FT transfers for the internal eth connector by [@karim-en]. ([#922])

[#922]: https://github.com/aurora-is-near/aurora-engine/pull/922

## [3.6.2] 2024-03-27

### Additions

- Added a new view transaction `ft_balances_of` for getting balances for multiple accounts by [@karim-en]. ([#905])

### Changes

- The `ft_resolve_transfer` callback doesn't require running the contract to finish the `ft_transfer_call` correctly
  by [@aleksuss]. ([#906])
- Borsh has been bumped to 1.3 what allows to get rid of additional feature `borsh-compat` by [@aleksuss]. ([#907])
- The `ExecutionProfile` has been extended with logs for tests by [@mrLSD]. ([#910])
- The interface of the engine standalone storage has been extended with a couple of methods for allowing set/get
  arbitrary data outside the crate by [@aleksuss]. ([#913])

### Fixes

- Minor improvements and fixes by [@raventid]. ([#916])

[#905]: https://github.com/aurora-is-near/aurora-engine/pull/905
[#906]: https://github.com/aurora-is-near/aurora-engine/pull/906
[#907]: https://github.com/aurora-is-near/aurora-engine/pull/907
[#910]: https://github.com/aurora-is-near/aurora-engine/pull/910
[#913]: https://github.com/aurora-is-near/aurora-engine/pull/913
[#916]: https://github.com/aurora-is-near/aurora-engine/pull/916 

## [3.6.1] 2024-02-15

### Changes

- Improved the format of a panic message by extending it with nonces from an account and a transaction
  by [@aleksuss]. ([#898]) 

[#898]: https://github.com/aurora-is-near/aurora-engine/pull/898

## [3.6.0] 2024-02-06

### Fixes

- Fixed underflow in the modexp gas calculation by [@guidovranken]. ([#883])
- Prevented subtraction underflow in the xcc module by [@guidovranken]. ([#888])
- Fixed balance and gas overflows in the xcc module by [@guidovranken]. ([#889])

### Changes

- CI was updated by changing self-hosted runner to the GitHub heavy by [@aleksuss]. ([#881])
- Removed a logic of fee calculation in the eth-connector by [@karim-en]. ([#882])
- Version of the rust nightly was updated to 2023-12-15 by [@RomanHodulak]. ([#885])

[#881]: https://github.com/aurora-is-near/aurora-engine/pull/881
[#882]: https://github.com/aurora-is-near/aurora-engine/pull/882
[#883]: https://github.com/aurora-is-near/aurora-engine/pull/883
[#885]: https://github.com/aurora-is-near/aurora-engine/pull/885
[#888]: https://github.com/aurora-is-near/aurora-engine/pull/888
[#889]: https://github.com/aurora-is-near/aurora-engine/pull/889

## [3.5.0] 2023-12-06

### Additions

- Added a new transaction `upgrade` which allows upgrading the contract and invoking the `state_migration` callback
  with one call by [@aleksuss]. ([#878])

### Fixes

- Updated the logic of upgrading XCC router which works properly on both `mainnet` and `testnet` by [@birchmd]. ([#877])  

[#877]: https://github.com/aurora-is-near/aurora-engine/pull/877
[#878]: https://github.com/aurora-is-near/aurora-engine/pull/878

## [3.4.0] 2023-11-28

### Additions

- Added a possibility to pass initialize arguments in json format to the `new` transaction by [@aleksuss]. ([#871])
- The `SubmitResult` was made available for `ft_on_transfer` transactions in the standalone engine by [@birchmd]. ([#869])
- The order of producing the exit precompile and XCC promises has been changed to sequential by [@birchmd]. ([#868])

### Changes

- Removed the code hidden behind the feature that isn't used anymore by [@joshuajbouw]. ([#870])
- The logic of unwrapping wNEAR has been changed to the Bridge's native by [@birchmd]. ([#867])
- Bumped the `near-workspaces` to 0.9 by [@aleksuss]. ([#862])

### Fixes

- Add a method for upgrading XCC router contract by [@birchmd]. ([#866])
- Fixed a potential panic in the `ExitToNear` precompile by [@guidovranken]. ([#865])
- Fixed a behaviour when the `ft_transfer` could occur before the `near_withdraw` by [@birchmd]. ([#864])
- Fixed correctness of reproducing the NEAR runtime random value in the standalone engine by [@birchmd]. ([#863])  

[#862]: https://github.com/aurora-is-near/aurora-engine/pull/862
[#863]: https://github.com/aurora-is-near/aurora-engine/pull/863
[#864]: https://github.com/aurora-is-near/aurora-engine/pull/864
[#865]: https://github.com/aurora-is-near/aurora-engine/pull/865
[#866]: https://github.com/aurora-is-near/aurora-engine/pull/866
[#867]: https://github.com/aurora-is-near/aurora-engine/pull/867
[#868]: https://github.com/aurora-is-near/aurora-engine/pull/868
[#869]: https://github.com/aurora-is-near/aurora-engine/pull/869
[#870]: https://github.com/aurora-is-near/aurora-engine/pull/870
[#871]: https://github.com/aurora-is-near/aurora-engine/pull/871

## [3.3.1] 2023-10-26

### Fixes

- The smart contract owner whose account id is not the same as the contract account id can set ERC-20 metadata
  by [@aleksuss]. ([#858]) 

[#858]: https://github.com/aurora-is-near/aurora-engine/pull/858

## [3.3.0] 2023-10-23

### Changes

- Changed the logic of cost calculation in Silo mode. Specifically, the fixed cost has been replaced with
  a fixed amount of gas per transaction by [@aleksuss]. ([#854]) 

- `near-workspaces-rs` has been updated to 0.8.0 by [@aleksuss]. ([#855])

[#854]: https://github.com/aurora-is-near/aurora-engine/pull/854
[#855]: https://github.com/aurora-is-near/aurora-engine/pull/855

## [3.2.0] 2023-10-17

### Changes

- Changed structure `SetEthConnectorContractAccountArgs` for setting eth connector account. It was extended with
 additional field: `withdraw_serialize_type` for defining serialization type for withdraw arguments by [@aleksuss]. ([#834])
- Updated rocksdb up to 0.21.0 by [@aleksuss]. ([#840])

### Additions

- Added a possibility of mirroring deployed ERC-20 contracts in the main Aurora contract in Silo mode by [@aleksuss]. ([#845])
- Allow to initialize hashchain directly with the `new` method by [@birchmd]. ([#846])
- Added a silo logic which allows to set fixed gas costs per transaction by [@aleksuss]. ([#746])
- Added a new type of transaction which allows to add full access key into account of the smart contract by [@aleksuss]. ([#847])

[#746]: https://github.com/aurora-is-near/aurora-engine/pull/746
[#834]: https://github.com/aurora-is-near/aurora-engine/pull/834
[#840]: https://github.com/aurora-is-near/aurora-engine/pull/840
[#845]: https://github.com/aurora-is-near/aurora-engine/pull/845
[#846]: https://github.com/aurora-is-near/aurora-engine/pull/846
[#847]: https://github.com/aurora-is-near/aurora-engine/pull/847

## [3.1.0] 2023-09-25

### Additions

- Added the possibility to use native NEAR instead of wNEAR on Aurora by [@karim-en]. ([#750])
- Added hashchain integration by [@birchmd]. ([#831])
- Added functions for setting and getting metadata of ERC-20 contracts deployed with `deploy_erc20_token` transaction
  by [@aleksuss]. ([#837])

[#750]: https://github.com/aurora-is-near/aurora-engine/pull/750
[#831]: https://github.com/aurora-is-near/aurora-engine/pull/831
[#837]: https://github.com/aurora-is-near/aurora-engine/pull/837

## [3.0.0] 2023-08-28

### Fixes

- Updated [SputnikVM](https://github.com/aurora-is-near/sputnikvm) dependency with bugfix in the `returndatacopy`
  implementation and a performance improvement in accessing EVM memory by [@birchmd]. ([#826])

### Changes

- BREAKING: `engine-standalone-storage` no longer automatically writes to the DB when `consume_message` is called. 
  It is up to downstream users of the library to commit the diff (after doing any validation for correctness) by [@birchmd]. ([#825])

### Additions

- New crate for the so-called "hashchain" implementation. It will enable verification of Aurora blocks by light clients 
  in the future by [@birchmd]. ([#816])

[#816]: https://github.com/aurora-is-near/aurora-engine/pull/816
[#825]: https://github.com/aurora-is-near/aurora-engine/pull/825
[#826]: https://github.com/aurora-is-near/aurora-engine/pull/826

## [2.10.2] 2023-08-10

### Changes

- Added a view transaction `factory_get_wnear_address` for returning address for the `wNEAR` ERC-20 contract by [@aleksuss]. ([#807]) 

### Fixes

- Fixed a bug where standalone engine can crash on tracing transactions with too large contract deployment by [@birchmd]. ([#817])
- Fixed a bug and performance improvements with unusual exponents in the `engine-modexp` crate by [@guidovranken]. ([#814]) 

[#817]: https://github.com/aurora-is-near/aurora-engine/pull/817
[#814]: https://github.com/aurora-is-near/aurora-engine/pull/814
[#807]: https://github.com/aurora-is-near/aurora-engine/pull/807

## [2.10.1] 2023-07-27

### Fixes

- Updated sputnikvm dependency with bugfix in the `shanghai` implementation [@mandreyel]. ([#803]) 

[#803]: https://github.com/aurora-is-near/aurora-engine/pull/803

## [2.10.0] 2023-07-20

### Changes

- Enabled Shanghai Ethereum hard fork support by [@mandreyel]. ([#773])
- Added ability to pause and resume the core functionality of the contract by [@Casuso]. ([#779])
- Added function call keys to be used with relayers by [@aleksuss]. ([#792])

[#773]: https://github.com/aurora-is-near/aurora-engine/pull/773
[#779]: https://github.com/aurora-is-near/aurora-engine/pull/779
[#792]: https://github.com/aurora-is-near/aurora-engine/pull/792

## [2.9.3] 2023-07-19

### Changes

- It is now possible for anyone to call the contract method `deploy_upgrade` by [@joshuajbouw]. ([#794])

[#794]: https://github.com/aurora-is-near/aurora-engine/pull/794

## [2.9.2] 2023-06-22

### Fixes

- Use ibig implementation of modexp by [@birchmd]. ([#778])

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
- Security improvement: only Engine contract owner can use the `deploy_upgrade` method by [@birchmd]. ([#410])
- Bug fix: Engine now returns the error message in the case of a revert during an EVM contract deploy, previously it would always return an address (even when the deploy failed) by [@birchmd]. ([#424])
- Security improvement: Engine will no longer accept EVM transactions without a chain ID as part of their signature by [@birchmd]. This should have no impact on users as all modern Ethereum tooling includes the chain ID. ([#432])
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

- Public method for computing Aurora blockhash at a given height by [@birchmd]. ([#303](https://github.com/aurora-is-near/aurora-engine/pull/303))

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

[Unreleased]: https://github.com/aurora-is-near/aurora-engine/compare/3.9.1...develop
[3.9.1]: https://github.com/aurora-is-near/aurora-engine/compare/3.9.0...3.9.1
[3.9.0]: https://github.com/aurora-is-near/aurora-engine/compare/3.8.0...3.9.0
[3.8.0]: https://github.com/aurora-is-near/aurora-engine/compare/3.7.0...3.8.0
[3.7.0]: https://github.com/aurora-is-near/aurora-engine/compare/3.6.4...3.7.0
[3.6.4]: https://github.com/aurora-is-near/aurora-engine/compare/3.6.3...3.6.4
[3.6.3]: https://github.com/aurora-is-near/aurora-engine/compare/3.6.2...3.6.3
[3.6.2]: https://github.com/aurora-is-near/aurora-engine/compare/3.6.1...3.6.2
[3.6.1]: https://github.com/aurora-is-near/aurora-engine/compare/3.6.0...3.6.1
[3.6.0]: https://github.com/aurora-is-near/aurora-engine/compare/3.5.0...3.6.0
[3.5.0]: https://github.com/aurora-is-near/aurora-engine/compare/3.4.0...3.5.0
[3.4.0]: https://github.com/aurora-is-near/aurora-engine/compare/3.3.1...3.4.0
[3.3.1]: https://github.com/aurora-is-near/aurora-engine/compare/3.3.0...3.3.1
[3.3.0]: https://github.com/aurora-is-near/aurora-engine/compare/3.2.0...3.3.0
[3.2.0]: https://github.com/aurora-is-near/aurora-engine/compare/3.1.0...3.2.0
[3.1.0]: https://github.com/aurora-is-near/aurora-engine/compare/3.0.0...3.1.0
[3.0.0]: https://github.com/aurora-is-near/aurora-engine/compare/3.0.0...2.10.2
[2.10.2]: https://github.com/aurora-is-near/aurora-engine/compare/2.10.1...2.10.2
[2.10.1]: https://github.com/aurora-is-near/aurora-engine/compare/2.10.0...2.10.1
[2.10.0]: https://github.com/aurora-is-near/aurora-engine/compare/2.9.3...2.10.0
[2.9.3]: https://github.com/aurora-is-near/aurora-engine/compare/2.9.2...2.9.3 
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
[@Casuso]: https://github.com/Casuso
[@DemoYeti]: https://github.com/DemoYeti
[@dwiekawki]: https://github.com/dwiekawki
[@guidovranken]: https://github.com/guidovranken
[@hskang9]: https://github.com/hskang9
[@joshuajbouw]: https://github.com/joshuajbouw
[@karim-en]: https://github.com/karim-en
[@mandreyel]: https://github.com/mandreyel
[@matklad]: https://github.com/matklad
[@mfornet]: https://github.com/mfornet
[@mrLSD]: https://github.com/mrLSD
[@olonho]: https://github.com/olonho
[@raventid]: https://github.com/raventid
[@RomanHodulak]: https://github.com/RomanHodulak
[@sept-en]: https://github.com/sept-en
[@spilin]: https://github.com/spilin
[@diegofigs]: https://github.com/diegofigs

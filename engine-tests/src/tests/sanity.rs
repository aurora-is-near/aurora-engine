use crate::prelude::{Address, U256};
use crate::prelude::{Wei, ERC20_MINT_SELECTOR};
use crate::utils::{self, str_to_account_id};
use aurora_engine::engine::{EngineErrorKind, GasPaymentError, ZERO_ADDRESS_FIX_HEIGHT};
use aurora_engine::parameters::{SetOwnerArgs, SetUpgradeDelayBlocksArgs, TransactionStatus};
use aurora_engine_sdk as sdk;
use aurora_engine_types::borsh::BorshDeserialize;
#[cfg(not(feature = "ext-connector"))]
use aurora_engine_types::parameters::connector::FungibleTokenMetadata;
use aurora_engine_types::H160;
use libsecp256k1::SecretKey;
use near_vm_runner::ContractCode;
use rand::RngCore;
use std::path::{Path, PathBuf};

const INITIAL_BALANCE: Wei = Wei::new_u64(1_000_000);
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: Wei = Wei::new_u64(123);
const GAS_PRICE: u64 = 10;

#[ignore]
#[test]
fn bench_memory_get_standalone() {
    let (mut runner, mut signer, _) = initialize_transfer();

    // This EVM program is an infinite loop which causes a large amount of memory to be
    // copied onto the EVM stack.
    let contract_bytes = vec![
        0x5b, 0x3a, 0x33, 0x43, 0x03, 0x59, 0x52, 0x59, 0x42, 0x59, 0x3a, 0x60, 0x05, 0x34, 0xf4,
        0x60, 0x33, 0x43, 0x05, 0x52, 0x56,
    ];
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            utils::create_deploy_transaction(contract_bytes, nonce)
        })
        .unwrap();
    let address = Address::try_from_slice(&utils::unwrap_success(result)).unwrap();

    runner.standalone_runner.as_mut().unwrap().env.block_height += 100;
    let tx = aurora_engine_transactions::legacy::TransactionLegacy {
        nonce: signer.use_nonce().into(),
        gas_price: U256::zero(),
        gas_limit: 10_000_000_u64.into(),
        to: Some(address),
        value: Wei::zero(),
        data: Vec::new(),
    };

    let start = std::time::Instant::now();
    let result = runner
        .standalone_runner
        .unwrap()
        .submit_transaction(&signer.secret_key, tx)
        .unwrap();
    let duration = start.elapsed().as_secs_f32();
    assert!(
        matches!(result.status, TransactionStatus::OutOfGas),
        "Infinite loops in the EVM run out of gas"
    );
    assert!(
        duration < 8.0,
        "Must complete this task in under 8s (in release build). Time taken: {duration} s",
    );
}

#[test]
fn test_returndatacopy() {
    let (mut runner, mut signer, _) = initialize_transfer();

    let deploy_contract = |runner: &mut utils::AuroraRunner,
                           signer: &mut utils::Signer,
                           contract_bytes: Vec<u8>|
     -> Address {
        let deploy = utils::create_deploy_transaction(contract_bytes, signer.use_nonce().into());
        let result = runner
            .submit_transaction(&signer.secret_key, deploy)
            .unwrap();
        Address::try_from_slice(&utils::unwrap_success(result)).unwrap()
    };

    let call_contract =
        |runner: &mut utils::AuroraRunner, signer: &mut utils::Signer, address: Address| {
            runner
                .submit_with_signer(signer, |nonce| {
                    aurora_engine_transactions::legacy::TransactionLegacy {
                        nonce,
                        gas_price: U256::zero(),
                        gas_limit: u64::MAX.into(),
                        to: Some(address),
                        value: Wei::zero(),
                        data: Vec::new(),
                    }
                })
                .unwrap()
        };

    // Call returndatacopy with len=0 and large memory offset (> u32::MAX)
    let contract_bytes = vec![0x60, 0x00, 0x3d, 0x33, 0x3e];
    let address = deploy_contract(&mut runner, &mut signer, contract_bytes);
    let result = call_contract(&mut runner, &mut signer, address);
    assert!(
        result.status.is_ok(),
        "EVM must handle returndatacopy with len=0"
    );

    // Call returndatacopy with len=1 and large memory offset (> u32::MAX)
    let contract_bytes = vec![0x60, 0x01, 0x3d, 0x33, 0x3e];
    let address = deploy_contract(&mut runner, &mut signer, contract_bytes);
    let result = call_contract(&mut runner, &mut signer, address);
    assert!(
        matches!(result.status, TransactionStatus::OutOfGas),
        "EVM must run out of gas if len > 0 with large memory offset"
    );
}

#[test]
fn test_total_supply_accounting() {
    let (mut runner, mut signer, benefactor) = initialize_transfer();

    let constructor = utils::solidity::ContractConstructor::compile_from_source(
        "src/tests/res",
        "target/solidity_build",
        "self_destructor.sol",
        "SelfDestruct",
    );

    let deploy_contract = |runner: &mut utils::AuroraRunner,
                           signer: &mut utils::Signer|
     -> utils::solidity::DeployedContract {
        let submit_result = runner
            .submit_with_signer(signer, |nonce| {
                let mut deploy_tx = constructor.deploy_without_constructor(nonce);
                deploy_tx.value = TRANSFER_AMOUNT;
                deploy_tx
            })
            .unwrap();

        let contract_address =
            Address::try_from_slice(utils::unwrap_success_slice(&submit_result)).unwrap();
        constructor.deployed_at(contract_address)
    };

    #[cfg(not(feature = "ext-connector"))]
    let get_total_supply = |runner: &utils::AuroraRunner| -> Wei {
        let result = runner
            .one_shot()
            .call("ft_total_eth_supply_on_aurora", "aurora", Vec::new());
        let amount: u128 = String::from_utf8(result.unwrap().return_data.as_value().unwrap())
            .unwrap()
            .replace('"', "")
            .parse()
            .unwrap();
        Wei::new(U256::from(amount))
    };

    // Self-destruct with some benefactor does not reduce the total supply
    let contract = deploy_contract(&mut runner, &mut signer);
    let _submit_result = runner
        .submit_with_signer(&mut signer, |nonce| {
            contract.call_method_with_args(
                "destruct",
                &[ethabi::Token::Address(ethabi::Address::from(
                    benefactor.raw().0,
                ))],
                nonce,
            )
        })
        .unwrap();
    assert_eq!(runner.get_balance(benefactor), TRANSFER_AMOUNT);
    #[cfg(not(feature = "ext-connector"))]
    assert_eq!(get_total_supply(&mut runner), INITIAL_BALANCE);

    // Self-destruct with self benefactor burns any ETH in the destroyed contract
    let contract = deploy_contract(&mut runner, &mut signer);
    let _submit_result = runner
        .submit_with_signer(&mut signer, |nonce| {
            contract.call_method_with_args(
                "destruct",
                &[ethabi::Token::Address(ethabi::Address::from(
                    contract.address.raw().0,
                ))],
                nonce,
            )
        })
        .unwrap();
    #[cfg(not(feature = "ext-connector"))]
    // For CANCUN hard fork `total_supply` can't change
    assert_eq!(get_total_supply(&mut runner), INITIAL_BALANCE);
}

#[test]
fn test_transaction_to_zero_address() {
    // Transactions that explicit list `0x0000...` as the `to` field in the transaction
    // should not be interpreted as contract creation. Previously this was the case
    // and it caused the Engine to incorrectly derive the sender's address.
    // See the mismatch between the sender address reported by the Aurora explorer
    // and the sender logged by the engine:
    //   - https://testnet.aurorascan.dev/tx/0x51846313113e13ff87ccbd153f1b339b857bf7729fe16af7d351ff06943c4c20
    //   - https://explorer.testnet.near.org/transactions/5URFuet378c6zokikG62uK4YH31AnZb99pDPRnVJBAy2
    // This is a test to show the bug is now fixed.
    let tx_hex = "f8648080836691b79400000000000000000000000000000000000000008080849c8a82caa0464cada9d6a907f5537dcc0f95274a30ddaeff33276e9b3993815586293a2010a07626bd794381ba59f30e26ec6f3448d19f63bb12dcda19acda429b2fb7d3dfba";
    let tx_bytes = hex::decode(tx_hex).unwrap();
    let tx = aurora_engine_transactions::EthTransactionKind::try_from(tx_bytes.as_slice()).unwrap();
    let normalized_tx = aurora_engine_transactions::NormalizedEthTransaction::try_from(tx).unwrap();
    let address = normalized_tx.address;
    let sender = hex::encode(address.as_bytes());
    assert_eq!(sender.as_str(), "63eafba871e0bda44be3cde19df5aa1c0f078142");

    // We want the standalone engine to still reproduce the old behaviour for blocks before the bug fix, and
    // to use the correct parsing for blocks after the fix.
    let mut runner = utils::standalone::StandaloneRunner::default();
    runner.init_evm_with_chain_id(normalized_tx.chain_id.unwrap());
    let mut context = utils::AuroraRunner::default().context;
    context.input = tx_bytes;
    // Prior to the fix the zero address is interpreted as None, causing a contract deployment.
    // It also incorrectly derives the sender address, so does not increment the right nonce.
    context.block_height = ZERO_ADDRESS_FIX_HEIGHT - 1;
    let result = runner
        .submit_raw(utils::SUBMIT, &context, &[], None)
        .unwrap();
    assert_eq!(result.gas_used, 53_000);
    runner.env.block_height = ZERO_ADDRESS_FIX_HEIGHT;
    assert_eq!(runner.get_nonce(&address), U256::zero());

    // After the fix this transaction is simply a transfer of 0 ETH to the zero address
    context.block_height = ZERO_ADDRESS_FIX_HEIGHT;
    let result = runner
        .submit_raw(utils::SUBMIT, &context, &[], None)
        .unwrap();
    assert_eq!(result.gas_used, 21_000);
    runner.env.block_height = ZERO_ADDRESS_FIX_HEIGHT + 1;
    assert_eq!(runner.get_nonce(&address), U256::one());
}

#[test]
fn test_state_format() {
    // The purpose of this test is to make sure that if we accidentally
    // change the binary format of the `EngineState` then we will know
    // about it. This is important because changing the state format will
    // break the contract unless we do a state migration.
    let args = aurora_engine::parameters::NewCallArgsV3 {
        chain_id: aurora_engine_types::types::u256_to_arr(&666.into()),
        owner_id: "boss".parse().unwrap(),
        upgrade_delay_blocks: 3,
        key_manager: "key_manager".parse().unwrap(),
    };
    let state: aurora_engine::state::EngineState = args.into();
    let expected_hex: String = [
        "02",                                                               // state version
        "000000000000000000000000000000000000000000000000000000000000029a", // chain id
        "04000000626f7373",                                                 // owner id
        "0300000000000000",                                                 // upgrade delay blocks
        "00",                                                               // contract mode
        "010b0000006b65795f6d616e61676572",                                 // key manager
    ]
    .concat();
    assert_eq!(hex::encode(state.borsh_serialize().unwrap()), expected_hex);
}

fn generate_code(len: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let mut buf = vec![0u8; len];
    rng.fill_bytes(&mut buf);
    buf
}

#[test]
fn test_deploy_contract() {
    let (mut runner, mut signer, _) = initialize_transfer();

    // Randomly generate some "contract code"
    let code = generate_code(567);
    // Deploy that code
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            utils::create_deploy_transaction(code.clone(), nonce)
        })
        .unwrap();
    let address = Address::try_from_slice(utils::unwrap_success_slice(&result)).unwrap();

    // Confirm the code stored at that address is equal to the input code.
    let stored_code = runner.get_code(address);
    assert_eq!(code, stored_code);
}

#[test]
fn test_deploy_largest_contract() {
    // Check to see we can deploy the largest allowed contract size within the
    // NEAR gas limit of 200 Tgas.
    let (mut runner, mut signer, _) = initialize_transfer();

    let len = evm::Config::berlin().create_contract_limit.unwrap();
    let code = generate_code(len);

    // Deploy that code
    let (result, profile) = runner
        .submit_with_signer_profiled(&mut signer, |nonce| {
            utils::create_deploy_transaction(code.clone(), nonce)
        })
        .unwrap();

    // At least 5 million EVM gas
    assert!(
        result.gas_used >= 5_000_000,
        "{:?} not greater than 5 million",
        result.gas_used,
    );

    // Less than 12 NEAR Tgas
    utils::assert_gas_bound(profile.all_gas(), 11);
}

#[test]
fn test_log_address() {
    let (mut runner, mut signer, _) = initialize_transfer();

    let mut deploy_contract = |name: &str, signer: &mut utils::Signer| {
        let constructor = utils::solidity::ContractConstructor::compile_from_source(
            "src/tests/res",
            "target/solidity_build",
            "caller.sol",
            name,
        );

        let nonce = signer.use_nonce();
        runner.deploy_contract(
            &signer.secret_key,
            |c| c.deploy_without_constructor(nonce.into()),
            constructor,
        )
    };

    let greet_contract = deploy_contract("Greeter", &mut signer);
    let caller_contract = deploy_contract("Caller", &mut signer);

    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            caller_contract.call_method_with_args(
                "greet",
                &[ethabi::Token::Address(ethabi::Address::from(
                    greet_contract.address.raw().0,
                ))],
                nonce,
            )
        })
        .unwrap();

    // Address included in the log should come from the contract emitting the log,
    // not the contract that invoked the call.
    let log_address = result.logs.first().unwrap().address;
    assert_eq!(log_address, greet_contract.address);
}

#[test]
fn test_is_contract() {
    let (mut runner, mut signer, _) = initialize_transfer();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);

    let constructor = utils::solidity::ContractConstructor::force_compile(
        "src/tests/res",
        "target/solidity_build",
        "is_contract.sol",
        "IsContract",
    );

    let nonce = signer.use_nonce();
    let contract = runner.deploy_contract(
        &signer.secret_key,
        |c| c.deploy_without_constructor(nonce.into()),
        constructor,
    );

    let call_contract =
        |account: Address, runner: &mut utils::AuroraRunner, signer: &mut utils::Signer| -> bool {
            let result = runner
                .submit_with_signer(signer, |nonce| {
                    contract.call_method_with_args(
                        "isContract",
                        &[ethabi::Token::Address(ethabi::Address::from(
                            account.raw().0,
                        ))],
                        nonce,
                    )
                })
                .unwrap();
            let bytes = utils::unwrap_success_slice(&result);
            ethabi::decode(&[ethabi::ParamType::Bool], bytes)
                .unwrap()
                .pop()
                .unwrap()
                .into_bool()
                .unwrap()
        };

    // Should return false for accounts that don't exist
    assert!(!call_contract(
        Address::from_array([1; 20]),
        &mut runner,
        &mut signer,
    ));

    // Should return false for accounts that don't have contract code
    assert!(!call_contract(signer_address, &mut runner, &mut signer),);

    // Should return true for contracts
    let erc20_constructor = utils::solidity::erc20::ERC20Constructor::load();
    let nonce = signer.use_nonce();
    let token_a = runner.deploy_contract(
        &signer.secret_key,
        |c| c.deploy("TOKEN_A", "TA", nonce.into()),
        erc20_constructor,
    );
    assert!(call_contract(token_a.address, &mut runner, &mut signer),);
}

#[test]
fn test_solidity_pure_bench() {
    let (mut runner, mut signer, _) = initialize_transfer();
    runner.max_gas_burnt(u64::MAX);

    let constructor = utils::solidity::ContractConstructor::force_compile(
        "src/tests/res",
        "target/solidity_build",
        "bench.sol",
        "Bencher",
    );

    let nonce = signer.use_nonce();
    let contract = runner.deploy_contract(
        &signer.secret_key,
        |c| c.deploy_without_constructor(nonce.into()),
        constructor,
    );

    // Number of iterations to do
    let loop_limit: u32 = 10_000;
    let (result, profile) = runner
        .submit_with_signer_profiled(&mut signer, |nonce| {
            contract.call_method_with_args(
                "cpu_ram_soak_test",
                &[ethabi::Token::Uint(loop_limit.into())],
                nonce,
            )
        })
        .unwrap();

    assert!(
        result.gas_used > 37_000_000,
        "Over 37 million EVM gas is used {}",
        result.gas_used
    );
    let near_gas = profile.all_gas();
    assert!(
        near_gas > 1400 * 1_000_000_000_000,
        "Expected 1500 NEAR Tgas to be used, but only consumed {}",
        near_gas / 1_000_000_000_000,
    );

    // Pure rust version of the same contract
    let base_path = Path::new("../etc").join("tests").join("benchmark-contract");
    let output_path =
        base_path.join("target/wasm32-unknown-unknown/release/benchmark_contract.wasm");
    utils::rust::compile(base_path);
    let contract_bytes = std::fs::read(output_path).unwrap();
    runner.set_code(ContractCode::new(contract_bytes, None));
    let mut context = runner.context.clone();
    context.input = loop_limit.to_le_bytes().to_vec();

    let contract = near_vm_runner::prepare(
        &runner.ext.underlying,
        runner.wasm_config.clone(),
        Some(&runner.cache),
        context.make_gas_counter(runner.wasm_config.as_ref()),
        "cpu_ram_soak_test",
    );

    let outcome = near_vm_runner::run(
        contract,
        &mut runner.ext,
        &context,
        runner.fees_config.clone(),
    )
    .unwrap();
    let profile = utils::ExecutionProfile::new(&outcome);

    // Check the contract actually did the work.
    assert_eq!(&outcome.logs, &[format!("Done {loop_limit} iterations!")]);
    assert!(profile.all_gas() < 1_000_000_000_000); // Less than 1 Tgas used!
}

#[test]
fn test_revert_during_contract_deploy() {
    let (mut runner, mut signer, _) = initialize_transfer();

    let constructor = utils::solidity::ContractConstructor::compile_from_source(
        "src/tests/res",
        "target/solidity_build",
        "reverter.sol",
        "ReverterByDefault",
    );

    let nonce = signer.use_nonce();
    let deploy_tx =
        constructor.deploy_with_args(nonce.into(), &[ethabi::Token::Uint(ethabi::Uint::zero())]);
    let submit_result = runner
        .submit_transaction(&signer.secret_key, deploy_tx)
        .unwrap();

    let revert_bytes = utils::unwrap_revert_slice(&submit_result);
    // First 4 bytes is a function selector with signature `Error(string)`
    assert_eq!(&revert_bytes[0..4], &[8, 195, 121, 160]);
    // Remaining data is an ABI-encoded string
    let revert_message = ethabi::decode(&[ethabi::ParamType::String], &revert_bytes[4..])
        .unwrap()
        .pop()
        .unwrap()
        .into_string()
        .unwrap();

    assert_eq!(revert_message.as_str(), "Revert message");
}

#[test]
fn test_call_too_deep_error() {
    let (mut runner, mut signer, _) = initialize_transfer();

    let constructor = utils::solidity::ContractConstructor::compile_from_source(
        "src/tests/res",
        "target/solidity_build",
        "CallTooDeep.sol",
        "CallTooDeep",
    );

    let nonce = signer.use_nonce();
    let contract = runner.deploy_contract(
        &signer.secret_key,
        |c| c.deploy_without_constructor(nonce.into()),
        constructor,
    );

    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            contract.call_method_without_args("test", nonce)
        })
        .unwrap();

    // It is counter-intuitive that this returns a `Revert` instead of `CallTooDeep`.
    // The reason this is the case is because it is only the last call that triggers the
    // `CallTooDeep` exit status, while the one before only sees that the call it made failed
    // and therefore reverts. As a result, the `CallTooDeep` exit status is not actually
    // visible to users.
    match result.status {
        TransactionStatus::Revert(_) => (),
        other => panic!("Unexpected status {other:?}"),
    }
}

#[test]
fn test_create_out_of_gas() {
    let (mut runner, mut signer, _) = initialize_transfer();
    let constructor_code = {
        // This EVM bytecode caused an integer overflow in the SputnikVM gas calculation prior to
        // https://github.com/aurora-is-near/sputnikvm/pull/17
        let code = "60e041184138410745413205374848484848484848484848484848484848484848484541384107456969697835ffff004545453a4747f06262483b646464645454543030303030303030303030303030303030303030303136383432353936337a5a5a8154543838929260545454545454545454315555555555555555555a5a5a5a5a5a5a5a3d5a615a5a5a5a5a455858580153455858585801825858828282545360305858586158f05858f05830303030303030303136383431353936337a5a5a8154543838929260545454545454545454315555555d55555555555a5a5a5a5a5a5a5a5a5a5a5a5a5a5a4558585801534558585858018258588282825453601558583158183d60253d60013a58f08258853480f07e82823aabac9fcdcea7a758583d6015315858585858f058585860253d60013a3d381a3d3361333030305858586158f05858f0583af00133303030828258588282825453601531585858583d60253d60013a58f08258853580f03a82827eab3d4343468546464646464646464646836500838311111111111111111111837676767676765a5a1515fb41151514742393f0555555555555555555555555555555555555555555555555555555555555555a5a5a5a5a5a455858580153455858585801827676765a5a1515fb41151514742393f055555555555555555555554558585801534558585858018258588282825453601531585858183d60253d60013a5858853580f03a82827eab3d9fcdcea7a75858fe3d60153f484848c40200000000000034483b325885858585858585853d60013a58f08261333030305858853580f03a82827eab30ac9fcdcea7a758583d6085853d60013a58f08261333060253d5e013a3d381a3d3361333030305858586158f05858f0583af001333030308282585882828254535a1531585858583d60253d60013a58f08258853580f03a82827eab3d9fcdcea7a758583d60153f484848483b323a4545314545353a4545450945317432454545304545304545303a4545314545353a45454509453174324545453045453a3a4545453a4545303a454530453a4545303a4545324545353a454545094531743a4546464646303a4545314545353a45454509453174324545453045453a3a4545453a4545303a454530453a4545303a4545314545353a4545450945317432454545304545304545303a4545314545353a45454509453174324545453045453a3a4545453a4545303a454530453a4545303a4545324545353a454545094531743a4546464619464646464646464646464646464646464646468258588282825453601531585858183d60253d60013a58f08258853580f03a828255555555555555555555555555555555555555555555555555556b6b6b6b3a5a3a4447474747f045456464ae646464646464646c6464325858435858013658584337585843015836585858384358585858f15858f158585885854085855858f15858f158580136585843375858430158f1585836585843385843385858013658584337585843015836585843585843385843385858013658584337585843015836585843385858585858f15858f158585858f15858f15858385858585858f15858f1585858f158585858f1585836585843385843385858365858015858433758f15858385858585858f15858f1585858f158585858f158583658584338584338585801365858489292605454545454545454543030303030303030303030303030303030303030303136383431353936337a5a5a8154543838929260545454545454545454315555555555555555555a5a5a5a5a5a5a5a5a5a5a5a413205374848484848484848484848484848484848485a6128a756455f07ef93f31ef468d3bc0d17e020b320616161616161616161616161616161616161616161616161515151515151070707070707070707070707070707070707070707070707070707070707070707070707070707070707070707070707070707070707070707070707070707070707070751515151484848485151515151515151515151515151515151515151515151515151515151515151515151515151515151515151515151518d616161616141364107454132053748484848484848489060604145614138415a07614541325a5a4558585801534558585858018258588282825453601531585858183d60253d60013a58f08258853480f07e82823aabac9fcdcea7a758583d6015315858585858f058585860253d60013a3d381a3d3361333030305858586158f05858f05830303030303030303136383431353936337a5a5a815454383892926054545454827676765a5a1515fb41151514742393f05555555555555555555555555555555555555555453a4747f04545646464646464646464646c6464643a474745343a4747f045454545453a4747f06262483b646464646464646464646c64646464646464646445646464646464646464646c6464646464646464f0305830303030343a36321a34347a36311a34347d34343a30282828282828282828282828282828282828282828282828282828282828a2a230340b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b050b0b0b3030303030303030303030303030303031353038323036333333333333333333333333333333333345304545304545303a4545314545353a45454509453174324545453045453a3a454506060606060606060606060606065050505050505050505050505050505050505050503361333030305858586158f05858f0581af00133303030828258588282825453601531585858583d60253d60013a58f28258853580f03a82827eab3d9fcdcea7a7464646464646464646464646462946464646464646464646464631707432454545304545353a4545453a4545303a4545304545353a453b32588585858585853a000000000000000000583f48383838486158f05858f0583af00133303030828258588282825453601531585858583d60253d60013a58f08258853580f03a82827eab3d9fcdcea7a758583d60153f484848483b32583f48d93838483b32586e858585858585585858f058585860253d5e013a3d381a3d33613330303058584358585858f15858f158585885854085855858f15858f158580136585843375858430158f1585836585843385843385858013658584337585843015836585843585843385843385858013658584337585843015836585843385858585858f15858f158585858f15858f15858385858585858f15858f1585858f158585858f1585836585843385843385858365858015858433758586158f05858f0583af00133305858586025603d013a3d381a5d3d3361050000003b325885b0bd02f6a392af548bdf1cfaee5dfa0eefcc8eab82827eabac9fcdcea7a758583dfeffffffffffffff6015000000000000000000000000ffffff303d389a3d603dff7effffffffffffff0600f15858385858585858f15858f1585858f158585858f158583658584338584338585801365858489292605454545454545454543030303030303030303030303030303030303030303136383431353936337a5a5a8154543838929260545454545454545454315555555555555555555a5a5a5a5a5affffffffffffffffffffffffffffffffffffffffffffffffffff5a5a5a5a5a5a5a5a5a4558585801534558585858018258588282825453601531585858183d60253d60013a58f08258853480f07e82823aabac9fcdcea7a758583d6015315858580000f70000000037201616355858f058585860253d60013a3d381a3d336133303030585851586158f05858f05830303030303030303136383431353936337a5a5a8154543838929260545454545454545454315555555555555555555a5a5a325a5a5a5a5a5a5a5a5a5a5a5a4558585801534558585858018258588282825453601558583158183d60253d60013a58f08258853480f07e82823aabac000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000009fcdcea7a758583d6015315858585858f058585860253d60013a3d381a3d3361333030305858586158f05858f0583af0013330303082825858828282545360153155555555555555555555555a5a5a5a5a5a4558585843468546464646464646464646838383111111111111111111ffff11837676767676765a5a1515fb41151514742393f0555555555555555555555555555555555555555555555555555555555555555a5a5a5a5a5a455858580153455858585801827676765a5a1515fb41151516742393f055555555555555555555555562483b45454545ff3a4747f06262483b4545454545453a47474745343a4747f045454555555555555555555555555555553d3d838311111111111111111111837676767676765a5a1515fb41151514742393f055483f3f453f484848483b32583f48383838483b3258858561616161616161616161616161616161615555555555555555555555555555555555555555555555556155555555618255555a82";
        hex::decode(code).unwrap()
    };
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            aurora_engine_transactions::legacy::TransactionLegacy {
                nonce,
                gas_price: U256::zero(),
                gas_limit: u64::MAX.into(),
                to: None,
                value: Wei::zero(),
                data: constructor_code,
            }
        })
        .unwrap();
    assert!(
        matches!(result.status, TransactionStatus::OutOfGas),
        "Unexpected status: {:?}",
        result.status
    );
}

#[test]
fn test_timestamp() {
    let (mut runner, mut signer, _) = initialize_transfer();

    let constructor = utils::solidity::ContractConstructor::compile_from_source(
        "src/tests/res",
        "target/solidity_build",
        "timestamp.sol",
        "Timestamp",
    );

    // deploy contract
    let nonce = signer.use_nonce();
    let contract = runner.deploy_contract(
        &signer.secret_key,
        |c| c.deploy_without_constructor(nonce.into()),
        constructor,
    );

    // set timestamp
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let nanos = t.as_nanos();
    let secs = U256::from(t.as_secs());
    runner.context.block_timestamp = u64::try_from(nanos).unwrap();

    // call contract
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            contract.call_method_without_args("getCurrentBlockTimestamp", nonce)
        })
        .unwrap();
    let timestamp = U256::from_big_endian(&utils::unwrap_success(result));

    // Check time is correct.
    // The `+1`  is needed here because the runner increments the context
    // timestamp by 1 second automatically before each transaction.
    assert_eq!(secs + 1, timestamp);
}

#[test]
fn test_override_state() {
    let (mut runner, mut account1, viewer_address) = initialize_transfer();
    let account1_address = utils::address_from_secret_key(&account1.secret_key);
    let mut account2 = utils::Signer::random();
    let account2_address = utils::address_from_secret_key(&account2.secret_key);
    runner.create_address(account2_address, INITIAL_BALANCE, INITIAL_NONCE.into());

    let contract = utils::solidity::ContractConstructor::compile_from_source(
        "src/tests/res",
        "target/solidity_build",
        "poster.sol",
        "Poster",
    );

    // deploy contract
    let result = runner
        .submit_with_signer(&mut account1, |nonce| {
            crate::prelude::transactions::legacy::TransactionLegacy {
                nonce,
                gas_price: U256::default(),
                gas_limit: u64::MAX.into(),
                to: None,
                value: Wei::default(),
                data: contract.code.clone(),
            }
        })
        .unwrap();
    let address = Address::try_from_slice(&utils::unwrap_success(result)).unwrap();
    let contract = contract.deployed_at(address);

    // define functions to interact with the contract
    let get_address = |runner: &utils::AuroraRunner| {
        let result = runner
            .view_call(&utils::as_view_call(
                contract.call_method_without_args("get", U256::zero()),
                viewer_address,
            ))
            .unwrap();
        match result {
            TransactionStatus::Succeed(bytes) => Address::try_from_slice(&bytes[12..32]).unwrap(),
            _ => panic!("tx failed"),
        }
    };

    let post_address = |runner: &mut utils::AuroraRunner, signer: &mut utils::Signer| {
        let result = runner
            .submit_with_signer(signer, |nonce| {
                contract.call_method_with_args(
                    "post",
                    &[ethabi::Token::String("Hello, world!".to_string())],
                    nonce,
                )
            })
            .unwrap();
        assert!(result.status.is_ok());
    };

    // Assert the initial state is 0
    assert_eq!(get_address(&runner), Address::new(H160([0; 20])));
    post_address(&mut runner, &mut account1);
    // Assert the address matches the first caller
    assert_eq!(get_address(&runner), account1_address);
    post_address(&mut runner, &mut account2);
    // Assert the address matches the second caller
    assert_eq!(get_address(&runner), account2_address);
}

#[test]
fn test_num_wasm_functions() {
    // Counts the number of functions in our wasm output.
    // See https://github.com/near/nearcore/issues/4814 for context
    let runner = utils::deploy_runner();
    let module = walrus::ModuleConfig::default()
        .parse(runner.ext.underlying.code.unwrap().code())
        .unwrap();
    let expected_number = 1600;
    let actual_number = module.funcs.iter().count();

    assert!(
        actual_number <= expected_number,
        "{actual_number} is not less than {expected_number}",
    );
}

/// Tests we can transfer Eth from one account to another and that the balances are correctly
/// updated.
#[test]
fn test_eth_transfer_success() {
    // set up Aurora runner and accounts
    let (mut runner, mut source_account, dest_address) = initialize_transfer();
    let source_address = utils::address_from_secret_key(&source_account.secret_key);

    // validate pre-state
    utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    )
    .unwrap();
    utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into())
        .unwrap();

    // perform transfer
    runner
        .submit_with_signer(&mut source_account, |nonce| {
            utils::transfer(dest_address, TRANSFER_AMOUNT, nonce)
        })
        .unwrap();

    // validate post-state
    utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE - TRANSFER_AMOUNT,
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
    utils::validate_address_balance_and_nonce(&runner, dest_address, TRANSFER_AMOUNT, 0.into())
        .unwrap();
}

/// Tests the case where the transfer amount is larger than the address balance
#[test]
fn test_eth_transfer_insufficient_balance() {
    let (mut runner, mut source_account, dest_address) = initialize_transfer();
    let source_address = utils::address_from_secret_key(&source_account.secret_key);

    // validate pre-state
    utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    )
    .unwrap();
    utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into())
        .unwrap();

    // attempt transfer
    let result = runner
        .submit_with_signer(&mut source_account, |nonce| {
            // try to transfer more than we have
            utils::transfer(dest_address, INITIAL_BALANCE + INITIAL_BALANCE, nonce)
        })
        .unwrap();
    assert_eq!(result.status, TransactionStatus::OutOfFund);

    // validate post-state
    utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        // the nonce is still incremented even though the transfer failed
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
    utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into())
        .unwrap();
}

/// Tests the case where the nonce on the transaction does not match the address
#[test]
fn test_eth_transfer_incorrect_nonce() {
    let (mut runner, mut source_account, dest_address) = initialize_transfer();
    let source_address = utils::address_from_secret_key(&source_account.secret_key);

    // validate pre-state
    utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    )
    .unwrap();
    utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into())
        .unwrap();

    // attempt transfer
    let error = runner
        .submit_with_signer(&mut source_account, |nonce| {
            // creating transaction with incorrect nonce
            utils::transfer(dest_address, TRANSFER_AMOUNT, nonce + 1)
        })
        .unwrap_err();
    assert!(
        matches!(error.kind, EngineErrorKind::IncorrectNonce(msg) if &msg == "ERR_INCORRECT_NONCE: ac: 0, tx: 1")
    );

    // validate post-state (which is the same as pre-state in this case)
    utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    )
    .unwrap();
    utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into())
        .unwrap();
}

#[test]
fn test_tx_support_shanghai() {
    let (mut runner, mut source_account, _) = initialize_transfer();
    // Encoded EVM transaction with parameter: `evmVersion: 'shanghai'`.
    let data = "6080604052348015600e575f80fd5b50607480601a5f395ff3fe6080604052348015600e575\
    f80fd5b50600436106026575f3560e01c8063919840ad14602a575b5f80fd5b600560405190815260200160\
    405180910390f3fea2646970667358221220cb01b9b9c75e5cd079a1980af2fe4397d2029888d12737d74cb\
    bc10e0de65bd364736f6c63430008150033";

    let result = runner
        .submit_with_signer(&mut source_account, |nonce| {
            aurora_engine_transactions::legacy::TransactionLegacy {
                nonce,
                gas_price: 0.into(),
                gas_limit: u64::MAX.into(),
                to: None,
                value: Wei::zero(),
                data: hex::decode(data).unwrap(),
            }
        })
        .expect("Should be able to execute EVM bytecode including PUSH0");

    assert!(result.status.is_ok());
}

#[test]
fn test_eth_transfer_not_enough_gas() {
    let (mut runner, mut source_account, dest_address) = initialize_transfer();
    let source_address = utils::address_from_secret_key(&source_account.secret_key);
    let transaction = |nonce| {
        let mut tx = utils::transfer(dest_address, TRANSFER_AMOUNT, nonce);
        tx.gas_limit = 10_000.into(); // this is not enough gas
        tx
    };

    // validate pre-state
    utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    )
    .unwrap();
    utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into())
        .unwrap();

    // attempt transfer
    let error = runner
        .submit_with_signer(&mut source_account, transaction)
        .unwrap_err();
    assert_eq!(error.kind, EngineErrorKind::IntrinsicGasNotMet);

    // validate post-state (which is the same as pre-state in this case)
    utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    )
    .unwrap();
    utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into())
        .unwrap();
}

#[test]
fn test_transfer_charging_gas_success() {
    let (mut runner, mut source_account, dest_address) = initialize_transfer();
    let source_address = utils::address_from_secret_key(&source_account.secret_key);
    let transaction = |nonce| {
        let mut tx = utils::transfer(dest_address, TRANSFER_AMOUNT, nonce);
        tx.gas_limit = 30_000.into();
        tx.gas_price = GAS_PRICE.into();
        tx
    };

    // validate pre-state
    utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    )
    .unwrap();
    utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into())
        .unwrap();

    // do transfer
    let result = runner
        .submit_with_signer(&mut source_account, transaction)
        .unwrap();
    let spent_amount = Wei::new_u64(GAS_PRICE * result.gas_used);
    let expected_source_balance = INITIAL_BALANCE - TRANSFER_AMOUNT - spent_amount;
    let expected_dest_balance = TRANSFER_AMOUNT;
    let expected_relayer_balance = spent_amount;
    let relayer_address =
        sdk::types::near_account_to_evm_address(runner.context.predecessor_account_id.as_bytes());

    // validate post-state
    utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        expected_source_balance,
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
    utils::validate_address_balance_and_nonce(
        &runner,
        dest_address,
        expected_dest_balance,
        0.into(),
    )
    .unwrap();
    utils::validate_address_balance_and_nonce(
        &runner,
        relayer_address,
        expected_relayer_balance,
        0.into(),
    )
    .unwrap();
}

#[test]
fn test_eth_transfer_charging_gas_not_enough_balance() {
    let (mut runner, mut source_account, dest_address) = initialize_transfer();
    let source_address = utils::address_from_secret_key(&source_account.secret_key);
    let transaction = |nonce| {
        let mut tx = utils::transfer(dest_address, TRANSFER_AMOUNT, nonce);
        // With this gas limit and price the account does not
        // have enough balance to cover the gas cost
        tx.gas_limit = 3_000_000.into();
        tx.gas_price = GAS_PRICE.into();
        tx
    };

    // validate pre-state
    utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    )
    .unwrap();
    utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into())
        .unwrap();

    // attempt transfer
    let error = runner
        .submit_with_signer(&mut source_account, transaction)
        .unwrap_err();
    assert_eq!(
        error.kind,
        EngineErrorKind::GasPayment(GasPaymentError::OutOfFund)
    );

    // validate post-state
    let relayer =
        sdk::types::near_account_to_evm_address(runner.context.predecessor_account_id.as_bytes());

    utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        // nonce is still not incremented since the transaction was invalid
        INITIAL_NONCE.into(),
    )
    .unwrap();
    utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into())
        .unwrap();
    utils::validate_address_balance_and_nonce(&runner, relayer, Wei::zero(), 0.into()).unwrap();
}

pub fn initialize_transfer() -> (utils::AuroraRunner, utils::Signer, Address) {
    // set up Aurora runner and accounts
    let mut runner = utils::deploy_runner();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    let source_address = utils::address_from_secret_key(&source_account);
    runner.create_address(source_address, INITIAL_BALANCE, INITIAL_NONCE.into());
    let dest_address = utils::address_from_secret_key(&SecretKey::random(&mut rng));
    let mut signer = utils::Signer::new(source_account);
    signer.nonce = INITIAL_NONCE;

    (runner, signer, dest_address)
}

#[test]
fn check_selector() {
    // Selector to call mint function in ERC-20 contract
    //
    // keccak("mint(address,uint256)".as_bytes())[..4];
    use sha3::Digest;
    let mut hasher = sha3::Keccak256::default();
    hasher.update(b"mint(address,uint256)");
    assert_eq!(hasher.finalize()[..4].to_vec(), ERC20_MINT_SELECTOR);
}

#[test]
fn test_block_hash() {
    let runner = utils::AuroraRunner::default();
    let chain_id = {
        let number = crate::prelude::U256::from(runner.chain_id);
        crate::prelude::u256_to_arr(&number)
    };
    let account_id = runner.aurora_account_id.as_bytes();
    let block_hash = aurora_engine::engine::compute_block_hash(chain_id, 10, account_id);

    assert_eq!(
        hex::encode(block_hash.0).as_str(),
        "c4a46f076b64877cbd8c5dbfd7bfbbea21a5653b79e3b6d06b6dfb5c88f1c384",
    );
}

#[test]
fn test_block_hash_api() {
    let runner = utils::deploy_runner();
    let block_height: u64 = 10;
    let outcome = runner
        .one_shot()
        .call(
            "get_block_hash",
            "any.near",
            borsh::to_vec(&block_height).unwrap(),
        )
        .unwrap();
    let block_hash = outcome.return_data.as_value().unwrap();

    assert_eq!(
        hex::encode(block_hash).as_str(),
        "c4a46f076b64877cbd8c5dbfd7bfbbea21a5653b79e3b6d06b6dfb5c88f1c384",
    );
}

#[test]
fn test_block_hash_contract() {
    let (mut runner, mut source_account, _) = initialize_transfer();
    let test_constructor = utils::solidity::ContractConstructor::compile_from_source(
        ["src", "tests", "res"].iter().collect::<PathBuf>(),
        Path::new("target").join("solidity_build"),
        "blockhash.sol",
        "BlockHash",
    );
    let nonce = source_account.use_nonce();
    let test_contract = runner.deploy_contract(
        &source_account.secret_key,
        |c| c.deploy_without_args(nonce.into()),
        test_constructor,
    );

    let result = runner
        .submit_with_signer(&mut source_account, |nonce| {
            test_contract.call_method_without_args("test", nonce)
        })
        .unwrap();

    let res = utils::panic_on_fail(result.status);
    assert!(res.is_none(), "Status: {res:?}");
}

#[cfg(not(feature = "ext-connector"))]
#[test]
fn test_ft_metadata() {
    let runner = utils::deploy_runner();
    let account_id: String = runner.context.signer_account_id.clone().into();
    let outcome = runner
        .one_shot()
        .call("ft_metadata", &account_id, Vec::new())
        .unwrap();
    let metadata =
        serde_json::from_slice::<FungibleTokenMetadata>(&outcome.return_data.as_value().unwrap())
            .unwrap();

    assert_eq!(metadata, FungibleTokenMetadata::default());
}

/// Tests transfer Eth from one account to another with custom argument `max_gas_price`.
#[test]
fn test_eth_transfer_with_max_gas_price() {
    // set up Aurora runner and accounts
    let (mut runner, source_account, dest_address) = initialize_transfer();
    let source_address = utils::address_from_secret_key(&source_account.secret_key);

    // validate pre-state
    utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE,
        INITIAL_NONCE.into(),
    )
    .unwrap();
    utils::validate_address_balance_and_nonce(&runner, dest_address, Wei::zero(), 0.into())
        .unwrap();

    // perform transfer
    let max_gas_price = 5;
    let mut transaction = utils::transfer(dest_address, TRANSFER_AMOUNT, INITIAL_NONCE.into());
    transaction.gas_price = 10.into();
    transaction.gas_limit = 30_000.into();

    let result = runner
        .submit_transaction_with_args(&source_account.secret_key, transaction, max_gas_price, None)
        .unwrap();

    let fee = u128::from(result.gas_used) * max_gas_price;
    // validate post-state
    utils::validate_address_balance_and_nonce(
        &runner,
        source_address,
        INITIAL_BALANCE - TRANSFER_AMOUNT - Wei::new_u128(fee),
        (INITIAL_NONCE + 1).into(),
    )
    .unwrap();
    utils::validate_address_balance_and_nonce(&runner, dest_address, TRANSFER_AMOUNT, 0.into())
        .unwrap();
}

#[test]
fn test_set_owner() {
    let mut runner = utils::deploy_runner();
    let aurora_account_id = runner.aurora_account_id.clone();

    // set owner args
    let set_owner_args = SetOwnerArgs {
        new_owner: str_to_account_id("new_owner.near"),
    };

    let result = runner.call(
        "set_owner",
        &aurora_account_id,
        borsh::to_vec(&set_owner_args).unwrap(),
    );

    // setting owner from the owner with same owner id should succeed
    assert!(result.is_ok());

    // get owner to see if the owner_id property has changed
    let outcome = runner
        .one_shot()
        .call("get_owner", &aurora_account_id, vec![])
        .unwrap();

    // check if the owner_id property has changed to new_owner.near
    assert_eq!(
        b"new_owner.near",
        outcome.return_data.as_value().unwrap().as_slice()
    );
}

#[test]
fn test_set_owner_fail_on_same_owner() {
    let mut runner = utils::deploy_runner();
    let aurora_account_id = runner.aurora_account_id.clone();

    // set owner args
    let set_owner_args = SetOwnerArgs {
        new_owner: str_to_account_id(&aurora_account_id),
    };

    let error = runner
        .call(
            "set_owner",
            &aurora_account_id,
            borsh::to_vec(&set_owner_args).unwrap(),
        )
        .unwrap_err();

    // check error equality
    assert_eq!(error.kind, EngineErrorKind::SameOwner);
}

#[test]
fn test_set_upgrade_delay_blocks() {
    let mut runner = utils::deploy_runner();
    let aurora_account_id = runner.aurora_account_id.clone();

    // set upgrade_delay_blocks args
    let set_upgrade_delay_blocks = SetUpgradeDelayBlocksArgs {
        upgrade_delay_blocks: 2,
    };

    let result = runner.call(
        "set_upgrade_delay_blocks",
        &aurora_account_id,
        borsh::to_vec(&set_upgrade_delay_blocks).unwrap(),
    );

    // should succeed
    assert!(result.is_ok());

    // get upgrade_delay_blocks to see if the upgrade_delay_blocks property has changed
    let result = runner
        .one_shot()
        .call("get_upgrade_delay_blocks", &aurora_account_id, vec![]);

    // check if the query goes through the standalone runner
    assert!(result.is_ok());

    // check if the upgrade_delay_blocks property has changed to 2
    let result = SetUpgradeDelayBlocksArgs::try_from_slice(
        result.unwrap().return_data.as_value().unwrap().as_slice(),
    )
    .unwrap();
    assert_eq!(result.upgrade_delay_blocks, 2);
}

mod workspace {
    use crate::prelude::{Address, U256};
    use crate::tests::sanity::{GAS_PRICE, INITIAL_BALANCE, INITIAL_NONCE, TRANSFER_AMOUNT};
    use crate::utils;
    use aurora_engine_types::parameters::engine::TransactionStatus;
    use aurora_engine_workspace::EngineContract;

    // Same as `test_eth_transfer_insufficient_balance` above, except runs through
    // `aurora-engine-workspace` instead of `near-vm-runner`. This is important because
    // `aurora-engine-workspace` has more production logic, in particular, state revert on
    // contract panic.
    // TODO: should be able to generalize the `call` backend of `AuroraRunner` so that this
    //       test does not need to be written twice.
    #[tokio::test]
    async fn test_eth_transfer_insufficient_balance() {
        let (aurora, mut signer, address) = initialize_engine().await;

        // Run transaction which will fail (transfer more than current balance)
        let nonce = signer.use_nonce();
        let tx = utils::transfer(
            Address::from_array([1; 20]),
            INITIAL_BALANCE + INITIAL_BALANCE,
            nonce.into(),
        );
        let signed_tx = utils::sign_transaction(
            tx,
            Some(utils::AuroraRunner::default().chain_id),
            &signer.secret_key,
        );

        let result = aurora
            .submit(rlp::encode(&signed_tx).to_vec())
            .transact()
            .await
            .unwrap()
            .into_value();
        assert_eq!(result.status, TransactionStatus::OutOfFund);

        // validate post-state
        assert_eq!(
            aurora.get_nonce(address).await.unwrap().result,
            (INITIAL_NONCE + 1).into(),
        );
        assert_eq!(
            aurora.get_balance(address).await.unwrap().result,
            INITIAL_BALANCE.raw(),
        );
    }

    // Same as `test_eth_transfer_charging_gas_not_enough_balance` but run through
    // `aurora-engine-workspace`.
    #[tokio::test]
    async fn test_eth_transfer_charging_gas_not_enough_balance() {
        let (aurora, mut signer, address) = initialize_engine().await;

        // Run transaction which will fail (not enough balance to cover gas)
        let nonce = signer.use_nonce();
        let mut tx = utils::transfer(Address::from_array([1; 20]), TRANSFER_AMOUNT, nonce.into());
        tx.gas_limit = 3_000_000.into();
        tx.gas_price = GAS_PRICE.into();
        let signed_tx = utils::sign_transaction(
            tx,
            Some(utils::AuroraRunner::default().chain_id),
            &signer.secret_key,
        );
        let error = aurora
            .submit(rlp::encode(&signed_tx).to_vec())
            .transact()
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("ERR_OUT_OF_FUND"));

        // validate post-state
        assert_eq!(
            aurora.get_nonce(address).await.unwrap().result,
            INITIAL_NONCE.into(), // nonce hasn't been changed because an error occurs
        );
        assert_eq!(
            aurora.get_balance(address).await.unwrap().result,
            INITIAL_BALANCE.raw(),
        );
    }

    async fn initialize_engine() -> (EngineContract, utils::Signer, Address) {
        let engine = utils::workspace::deploy_engine().await;
        let signer = utils::Signer::random();
        let address = utils::address_from_secret_key(&signer.secret_key);
        let result = engine
            .mint_account(address, INITIAL_NONCE, INITIAL_BALANCE.raw().low_u64())
            .max_gas()
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());

        // validate pre-state
        let nonce = engine.get_nonce(address).await.unwrap();
        assert_eq!(nonce.result, U256::from(INITIAL_NONCE),);

        let balance = engine.get_balance(address).await.unwrap();
        assert_eq!(balance.result, INITIAL_BALANCE.raw(),);

        (engine, signer, address)
    }
}

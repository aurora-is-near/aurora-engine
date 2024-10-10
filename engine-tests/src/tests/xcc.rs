use crate::utils::solidity::erc20::{ERC20Constructor, ERC20};
use crate::utils::{self, AuroraRunner, DEFAULT_AURORA_ACCOUNT_ID};
use aurora_engine_precompiles::xcc::{costs, cross_contract_call};
use aurora_engine_transactions::legacy::TransactionLegacy;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::borsh::BorshDeserialize;
use aurora_engine_types::parameters::{
    CrossContractCallArgs, NearPromise, PromiseArgs, PromiseCreateArgs, PromiseWithCallbackArgs,
    SimpleNearPromise,
};
use aurora_engine_types::types::{Address, EthGas, NearGas, Wei, Yocto};
use aurora_engine_types::U256;
use aurora_engine_workspace::types::NearToken;
use near_vm_runner::logic::mocks::mock_external::MockAction;
use near_vm_runner::ContractCode;
use std::fs;
use std::path::Path;

const WNEAR_AMOUNT: NearToken = NearToken::from_near(500);
const STORAGE_AMOUNT: NearToken = NearToken::from_near(50);

const XCC_ROUTER_BASE_PATH: &str = "../etc/xcc-router";
const XCC_ROUTER_VERSION_RELATIVE_PATH: &str = "src/VERSION";

#[test]
#[allow(clippy::too_many_lines)]
fn test_xcc_eth_gas_cost() {
    let mut runner = utils::deploy_runner();
    runner.standalone_runner = None;
    let xcc_wasm_bytes = contract_bytes();
    let _res = runner.call("factory_update", DEFAULT_AURORA_ACCOUNT_ID, xcc_wasm_bytes);
    let mut signer = utils::Signer::random();
    let mut baseline_signer = utils::Signer::random();
    // Skip to later block height and re-init hashchain
    let account_id = runner.aurora_account_id.clone();
    utils::init_hashchain(
        &mut runner,
        &account_id,
        Some(aurora_engine::engine::ZERO_ADDRESS_FIX_HEIGHT + 1),
    );

    // Need to use for engine's deployment!
    let wnear_erc20 = deploy_erc20(&mut runner, &signer);
    approve_erc20(
        &wnear_erc20,
        cross_contract_call::ADDRESS,
        &mut runner,
        &mut signer,
    );
    approve_erc20(
        &wnear_erc20,
        utils::address_from_secret_key(&baseline_signer.secret_key),
        &mut runner,
        &mut signer,
    );
    let _res = runner.call(
        "factory_set_wnear_address",
        DEFAULT_AURORA_ACCOUNT_ID,
        wnear_erc20.0.address.as_bytes().to_vec(),
    );

    // Baseline transaction is an ERC-20 transferFrom call since such a call is included as part
    // of the precompile execution, but we want to isolate just the precompile logic itself
    // (the EVM subcall is charged separately).
    let (baseline_result, baseline) = runner
        .submit_with_signer_profiled(&mut baseline_signer, |nonce| {
            wnear_erc20.transfer_from(
                utils::address_from_secret_key(&signer.secret_key),
                Address::from_array([1u8; 20]),
                U256::from(STORAGE_AMOUNT.as_yoctonear()),
                nonce,
            )
        })
        .unwrap();
    assert!(
        baseline_result.status.is_ok(),
        "Unexpected baseline status: {baseline_result:?}",
    );

    let mut profile_for_promise = |p: PromiseArgs| -> (u64, u64, u64) {
        let data = borsh::to_vec(&CrossContractCallArgs::Eager(p)).unwrap();
        let input_length = data.len();
        let (submit_result, profile) = runner
            .submit_with_signer_profiled(&mut signer, |nonce| TransactionLegacy {
                nonce,
                gas_price: U256::zero(),
                gas_limit: u64::MAX.into(),
                to: Some(cross_contract_call::ADDRESS),
                value: Wei::zero(),
                data,
            })
            .unwrap();
        assert!(submit_result.status.is_ok());
        // Subtract off baseline transaction to isolate just precompile things
        (
            u64::try_from(input_length).unwrap(),
            profile.all_gas() - baseline.all_gas(),
            submit_result.gas_used,
        )
    };

    let promise = PromiseCreateArgs {
        target_account_id: "some_account.near".parse().unwrap(),
        method: "some_method".into(),
        args: b"hello_world".to_vec(),
        attached_balance: Yocto::new(56),
        attached_gas: NearGas::new(500),
    };
    // Shorter input
    let (x1, y1, evm1) = profile_for_promise(PromiseArgs::Create(promise.clone()));
    // longer input
    let (x2, y2, evm2) = profile_for_promise(PromiseArgs::Callback(PromiseWithCallbackArgs {
        base: promise.clone(),
        callback: promise,
    }));

    // NEAR costs (inferred from a line through (x1, y1) and (x2, y2))
    let xcc_cost_per_byte = (y2 - y1) / (x2 - x1);
    let xcc_base_cost = NearGas::new(y1 - xcc_cost_per_byte * x1);

    // Convert to EVM cost using conversion ratio
    let xcc_base_cost = EthGas::new(xcc_base_cost.as_u64() / costs::CROSS_CONTRACT_CALL_NEAR_GAS);
    let xcc_cost_per_byte = xcc_cost_per_byte / costs::CROSS_CONTRACT_CALL_NEAR_GAS;

    assert!(
        utils::within_x_percent(
            5,
            xcc_base_cost.as_u64(),
            costs::CROSS_CONTRACT_CALL_BASE.as_u64(),
        ),
        "Incorrect xcc base cost. Expected: {} Actual: {}",
        xcc_base_cost,
        costs::CROSS_CONTRACT_CALL_BASE
    );

    assert!(
        utils::within_x_percent(
            5,
            xcc_cost_per_byte,
            costs::CROSS_CONTRACT_CALL_BYTE.as_u64(),
        ),
        "Incorrect xcc per byte cost. Expected: {} Actual: {}",
        xcc_cost_per_byte,
        costs::CROSS_CONTRACT_CALL_BYTE
    );

    // As a sanity check, confirm that the total EVM gas spent aligns with expectations.
    // The additional gas added is the amount attached to the XCC call (this is "used", but not
    // "burnt").
    let total_gas1 = y1 + baseline.all_gas() + costs::ROUTER_EXEC_BASE.as_u64();
    let total_gas2 = y2
        + baseline.all_gas()
        + costs::ROUTER_EXEC_BASE.as_u64()
        + costs::ROUTER_EXEC_PER_CALLBACK.as_u64();
    assert!(
        utils::within_x_percent(20, evm1, total_gas1 / costs::CROSS_CONTRACT_CALL_NEAR_GAS),
        "Incorrect EVM gas used. Expected: {} Actual: {}",
        evm1,
        total_gas1 / costs::CROSS_CONTRACT_CALL_NEAR_GAS
    );
    assert!(
        utils::within_x_percent(20, evm2, total_gas2 / costs::CROSS_CONTRACT_CALL_NEAR_GAS),
        "Incorrect EVM gas used. Expected: {} Actual: {}",
        evm2,
        total_gas2 / costs::CROSS_CONTRACT_CALL_NEAR_GAS
    );
}

fn check_fib_result(output: &serde_json::Value, n: usize) {
    let fib_numbers: [u8; 8] = [0, 1, 1, 2, 3, 5, 8, 13];
    let get_number = |field_name: &str| -> u8 {
        output
            .as_object()
            .unwrap()
            .get(field_name)
            .unwrap()
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    };
    let a = get_number("a");
    let b = get_number("b");
    assert_eq!(a, fib_numbers[n]);
    assert_eq!(b, fib_numbers[n + 1]);
}

#[test]
fn test_xcc_schedule_gas() {
    let mut router = deploy_router();

    let promise = PromiseCreateArgs {
        target_account_id: "some_account.near".parse().unwrap(),
        method: "some_method".into(),
        args: b"hello_world".to_vec(),
        attached_balance: Yocto::new(56),
        attached_gas: NearGas::new(100_000_000_000_000),
    };

    let outcome = router
        .call(
            "schedule",
            DEFAULT_AURORA_ACCOUNT_ID,
            borsh::to_vec(&PromiseArgs::Create(promise)).unwrap(),
        )
        .unwrap();
    assert!(
        outcome.burnt_gas < costs::ROUTER_SCHEDULE.as_u64(),
        "{:?} not less than {:?}",
        outcome.burnt_gas,
        costs::ROUTER_SCHEDULE
    );
    assert_eq!(outcome.logs.len(), 1);
    assert_eq!(outcome.logs[0], "Promise scheduled at nonce 0");
}

#[test]
fn test_xcc_exec_gas() {
    let mut router = deploy_router();
    router.ext.underlying.action_log.clear(); // we need to clear logs here to remove old entries.

    let create_promise_chain = |base_promise: &PromiseCreateArgs, callback_count| -> NearPromise {
        (0..callback_count).fold(
            NearPromise::Simple(SimpleNearPromise::Create(base_promise.clone())),
            |result, _| NearPromise::Then {
                base: Box::new(result),
                callback: SimpleNearPromise::Create(base_promise.clone()),
            },
        )
    };

    let promise = PromiseCreateArgs {
        target_account_id: "some_account.near".parse().unwrap(),
        method: "some_method".into(),
        args: b"hello_world".to_vec(),
        attached_balance: Yocto::new(56),
        attached_gas: NearGas::new(100_000_000_000_000),
    };

    for callback_count in 0..5 {
        let x = create_promise_chain(&promise, callback_count);
        let args = PromiseArgs::Recursive(x);

        let outcome = router
            .call(
                "execute",
                DEFAULT_AURORA_ACCOUNT_ID,
                borsh::to_vec(&args).unwrap(),
            )
            .unwrap();
        let callback_count = args.promise_count() - 1;
        let router_exec_cost = costs::ROUTER_EXEC_BASE
            + NearGas::new(callback_count * costs::ROUTER_EXEC_PER_CALLBACK.as_u64());
        assert!(
            outcome.burnt_gas < router_exec_cost.as_u64(),
            "{:?} not less than {:?}",
            outcome.burnt_gas,
            router_exec_cost
        );

        for action in &router.ext.underlying.action_log {
            match action {
                MockAction::FunctionCallWeight {
                    method_name,
                    args,
                    attached_deposit,
                    prepaid_gas,
                    ..
                } => {
                    assert_eq!(method_name, promise.method.as_bytes());
                    assert_eq!(args, &promise.args);
                    assert_eq!(attached_deposit, &promise.attached_balance.as_u128());
                    assert_eq!(prepaid_gas, &promise.attached_gas.as_u64());
                }
                MockAction::CreateReceipt { receiver_id, .. } => {
                    assert_eq!(receiver_id.as_bytes(), promise.target_account_id.as_bytes());
                }
                other => panic!("Unexpected action {other:?}"),
            };
        }

        router.ext.underlying.action_log.clear();
    }
}

fn deploy_router() -> AuroraRunner {
    let mut router = AuroraRunner::default();
    router.set_code(ContractCode::new(contract_bytes(), None));

    // Standalone not relevant here because this is not an Aurora Engine instance
    router.standalone_runner = None;
    router.context.current_account_id = "some_address.aurora".parse().unwrap();
    router.context.predecessor_account_id = DEFAULT_AURORA_ACCOUNT_ID.parse().unwrap();

    let init_args = r#"{"wnear_account": "wrap.near", "must_register": true}"#;
    let outcome = router
        .call(
            "initialize",
            DEFAULT_AURORA_ACCOUNT_ID,
            init_args.as_bytes().to_vec(),
        )
        .unwrap();
    assert!(outcome.used_gas < aurora_engine::xcc::INITIALIZE_GAS.as_u64());

    router
}

fn deploy_erc20(runner: &mut AuroraRunner, signer: &utils::Signer) -> ERC20 {
    let engine_account = runner.aurora_account_id.clone();
    let args = aurora_engine::parameters::DeployErc20TokenArgs {
        nep141: "wrap.near".parse().unwrap(),
    };
    let outcome = runner
        .call(
            "deploy_erc20_token",
            &engine_account,
            borsh::to_vec(&args).unwrap(),
        )
        .unwrap();
    let address = {
        let bytes: Vec<u8> =
            BorshDeserialize::try_from_slice(outcome.return_data.as_value().as_ref().unwrap())
                .unwrap();
        Address::try_from_slice(&bytes).unwrap()
    };

    let contract = ERC20(ERC20Constructor::load().0.deployed_at(address));
    let dest_address = utils::address_from_secret_key(&signer.secret_key);
    let call_args =
        aurora_engine::parameters::CallArgs::V1(aurora_engine::parameters::FunctionCallArgsV1 {
            contract: address,
            input: contract
                .mint(
                    dest_address,
                    WNEAR_AMOUNT.as_yoctonear().into(),
                    U256::zero(),
                )
                .data,
        });
    let result = runner.call("call", &engine_account, borsh::to_vec(&call_args).unwrap());
    assert!(result.is_ok());

    contract
}

fn approve_erc20(
    token: &ERC20,
    spender: Address,
    runner: &mut AuroraRunner,
    signer: &mut utils::Signer,
) {
    let approve_result = runner
        .submit_with_signer(signer, |nonce| {
            token.approve(spender, WNEAR_AMOUNT.as_yoctonear().into(), nonce)
        })
        .unwrap();
    assert!(approve_result.status.is_ok());
}

pub fn contract_bytes() -> Vec<u8> {
    let base_path = Path::new(XCC_ROUTER_BASE_PATH);
    let output_path = base_path.join("target/wasm32-unknown-unknown/release/xcc_router.wasm");
    utils::rust::compile(base_path);
    fs::read(output_path).unwrap()
}

pub fn router_version() -> u32 {
    let base_path = Path::new(XCC_ROUTER_BASE_PATH);
    let file_path = base_path.join(XCC_ROUTER_VERSION_RELATIVE_PATH);
    let version = fs::read_to_string(file_path).unwrap();
    version.trim().parse().unwrap()
}

pub fn change_router_version(version: u32) {
    let base_path = Path::new(XCC_ROUTER_BASE_PATH);
    let file_path = base_path.join(XCC_ROUTER_VERSION_RELATIVE_PATH);
    fs::write(file_path, format!("{version}\n")).unwrap();
}

fn make_fib_promise(n: usize, account_id: &AccountId) -> NearPromise {
    if n == 0 {
        NearPromise::Simple(SimpleNearPromise::Create(PromiseCreateArgs {
            target_account_id: account_id.clone(),
            method: "seed".into(),
            args: Vec::new(),
            attached_balance: Yocto::new(0),
            attached_gas: NearGas::new(5_000_000_000_000),
        }))
    } else {
        let base = make_fib_promise(n - 1, account_id);
        let callback = SimpleNearPromise::Create(PromiseCreateArgs {
            target_account_id: account_id.clone(),
            method: "accumulate".into(),
            args: Vec::new(),
            attached_balance: Yocto::new(0),
            attached_gas: NearGas::new(5_000_000_000_000),
        });
        NearPromise::Then {
            base: Box::new(base),
            callback,
        }
    }
}

pub mod workspace {
    use crate::tests::xcc::{check_fib_result, WNEAR_AMOUNT};
    use crate::utils;
    use crate::utils::solidity::erc20::{ERC20Constructor, ERC20};
    use crate::utils::workspace::{
        create_sub_account, deploy_engine, deploy_engine_v331, deploy_erc20_from_nep_141,
        deploy_nep_141, get_xcc_router_version, nep_141_balance_of, transfer_nep_141_to_erc_20,
    };
    use aurora_engine_precompiles::xcc::cross_contract_call;
    use aurora_engine_transactions::legacy::TransactionLegacy;
    use aurora_engine_types::account_id::AccountId;
    use aurora_engine_types::parameters::engine::TransactionStatus;
    use aurora_engine_types::parameters::{
        CrossContractCallArgs, NearPromise, PromiseArgs, PromiseCreateArgs,
        PromiseWithCallbackArgs, SimpleNearPromise,
    };
    use aurora_engine_types::types::{Address, NearGas, Wei, Yocto};
    use aurora_engine_types::U256;
    use aurora_engine_workspace::types::NearToken;
    use aurora_engine_workspace::{EngineContract, RawContract};
    use serde_json::json;
    use std::path::Path;

    const STORAGE_AMOUNT: NearToken = NearToken::from_near(50);
    const ONE_NEAR: u128 = NearToken::from_near(1).as_yoctonear();

    #[tokio::test]
    async fn test_xcc_external_fund() {
        // In this test we intentionally do not bridge wNEAR into the Engine.
        // The purpose of the `fund_xcc_sub_account` functionality is to allow using
        // the XCC feature in an Engine instance where there is no bridged wNEAR.

        // Set up Engine contract
        let aurora = deploy_engine().await;
        let chain_id = aurora.get_chain_id().await.unwrap().result.as_u64();
        let mut signer = utils::Signer::new(libsecp256k1::SecretKey::parse(&[0xab; 32]).unwrap());
        let signer_address = utils::address_from_secret_key(&signer.secret_key);
        let xcc_wasm_bytes = super::contract_bytes();

        let result = aurora
            .factory_update(xcc_wasm_bytes)
            .max_gas()
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());

        let wnear_account = deploy_wnear(&aurora).await.unwrap();

        // Fund XCC sub-account
        let fund_amount = NearToken::from_near(5);
        let result = aurora
            .fund_xcc_sub_account(
                signer_address,
                Some(wnear_account.id().as_ref().parse().unwrap()),
            )
            .max_gas()
            .deposit(fund_amount)
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());

        let sub_account_id = format!("{}.{}", signer_address.encode(), aurora.id().as_ref());
        let sub_account_balance = aurora
            .node
            .get_balance(&sub_account_id.parse().unwrap())
            .await
            .unwrap();
        assert_eq!(
            (fund_amount.as_yoctonear() - sub_account_balance) / ONE_NEAR,
            0
        );

        // Do an XCC call. This XCC call is to the Aurora Engine itself to deploy an EVM contract,
        // but that is just for this test. The call could be to any contract to do any action.
        let expected_code = hex::decode("deadbeef").unwrap();
        let deploy_code =
            utils::create_deploy_transaction(expected_code.clone(), U256::zero()).data;
        let promise = PromiseCreateArgs {
            target_account_id: aurora.id().as_ref().parse().unwrap(),
            method: "deploy_code".into(),
            args: deploy_code,
            attached_balance: Yocto::new(0),
            attached_gas: NearGas::new(10_000_000_000_000),
        };
        let xcc_args = CrossContractCallArgs::Eager(PromiseArgs::Create(promise));
        let result = submit_xcc_transaction(&xcc_args, &aurora, &mut signer, chain_id).await;
        assert!(result.is_ok(), "{:?}", result.err());

        // This is known because we are using a fixed private key for the signer
        let deployed_address = Address::decode("bda6e7f87c816d25718c38b1c753e280f9455350").unwrap();
        let code = aurora.get_code(deployed_address).await.unwrap().result;

        assert_eq!(
            code, expected_code,
            "Failed to properly deploy EVM code via XCC"
        );
    }

    #[tokio::test]
    async fn test_xcc_precompile_eager() {
        test_xcc_precompile_common(false).await;
    }

    #[tokio::test]
    async fn test_xcc_precompile_scheduled() {
        test_xcc_precompile_common(true).await;
    }

    /// This test uses the XCC feature where the promise has many nested callbacks.
    /// The contract it uses is one which computes Fibonacci numbers in an inefficient way.
    /// The contract has two functions: `seed` and `accumulate`.
    /// The `seed` function always returns `{"a": "0", "b": "1"}`.
    /// The `accumulate` function performs one step of the Fibonacci recursion relation using
    /// a promise result (i.e. result from prior call) as input.
    /// Therefore, we can compute Fibonacci numbers by creating a long chain of callbacks.
    /// For example, to compute the 6th number:
    /// `seed.then(accumulate).then(accumulate).then(accumulate).then(accumulate).then(accumulate)`.
    #[tokio::test]
    async fn test_xcc_multiple_callbacks() {
        let XccTestContext {
            aurora,
            mut signer,
            signer_address,
            chain_id,
            ..
        } = init_xcc().await.unwrap();

        // 1. Deploy Fibonacci contract
        let fib_account_id = deploy_fibonacci(&aurora).await.unwrap();

        // 2. Create XCC account, schedule Fibonacci call
        let n = 6;
        let promise = super::make_fib_promise(n, &fib_account_id);
        let xcc_args = CrossContractCallArgs::Delayed(PromiseArgs::Recursive(promise));
        let result = submit_xcc_transaction(&xcc_args, &aurora, &mut signer, chain_id).await;
        assert!(result.is_ok(), "{:?}", result.err());

        // 3. Make Fibonacci call
        let router_account = format!(
            "{}.{}",
            hex::encode(signer_address.as_bytes()),
            aurora.id().as_ref()
        );
        let result = aurora
            .root()
            .call(&router_account.parse().unwrap(), "execute_scheduled")
            .args_json(json!({"nonce": "0"}))
            .max_gas()
            .transact()
            .await
            .unwrap();
        assert!(result.is_success(), "{result:?}");

        // 4. Check the result is correct
        let output = result.json().unwrap();
        check_fib_result(&output, n);
    }

    // This test is similar to `test_xcc_multiple_callbacks`, but instead of computing
    // Fibonacci numbers through repeated callbacks, it uses the `And` promise combinator.
    #[tokio::test]
    async fn test_xcc_and_combinator() {
        let XccTestContext {
            aurora,
            mut signer,
            signer_address,
            chain_id,
            ..
        } = init_xcc().await.unwrap();

        // 1. Deploy Fibonacci contract
        let fib_account_id = deploy_fibonacci(&aurora).await.unwrap();

        // 2. Create XCC account, schedule Fibonacci call
        let n = 6;
        let promise = NearPromise::Then {
            base: Box::new(NearPromise::And(vec![
                NearPromise::Simple(SimpleNearPromise::Create(PromiseCreateArgs {
                    target_account_id: fib_account_id.clone(),
                    method: "fib".into(),
                    args: format!(r#"{{"n": {}}}"#, n - 1).into_bytes(),
                    attached_balance: Yocto::new(0),
                    attached_gas: NearGas::new(10_000_000_000_000_u64 * n),
                })),
                NearPromise::Simple(SimpleNearPromise::Create(PromiseCreateArgs {
                    target_account_id: fib_account_id.clone(),
                    method: "fib".into(),
                    args: format!(r#"{{"n": {}}}"#, n - 2).into_bytes(),
                    attached_balance: Yocto::new(0),
                    attached_gas: NearGas::new(10_000_000_000_000_u64 * n),
                })),
            ])),
            callback: SimpleNearPromise::Create(PromiseCreateArgs {
                target_account_id: fib_account_id,
                method: "sum".into(),
                args: Vec::new(),
                attached_balance: Yocto::new(0),
                attached_gas: NearGas::new(5_000_000_000_000),
            }),
        };
        let xcc_args = CrossContractCallArgs::Delayed(PromiseArgs::Recursive(promise));
        let result = submit_xcc_transaction(&xcc_args, &aurora, &mut signer, chain_id).await;
        assert!(result.is_ok(), "{:?}", result.err());

        // 3. Make Fibonacci call
        let router_account = format!(
            "{}.{}",
            hex::encode(signer_address.as_bytes()),
            aurora.id().as_ref()
        );
        let result = aurora
            .root()
            .call(&router_account.parse().unwrap(), "execute_scheduled")
            .args_json(json!({"nonce": "0"}))
            .max_gas()
            .transact()
            .await
            .unwrap();
        assert!(result.is_success(), "{result:?}");

        // 4. Check the result is correct
        let output = result.json().unwrap();
        check_fib_result(&output, usize::try_from(n).unwrap());
    }

    #[allow(clippy::too_many_lines)]
    #[tokio::test]
    async fn test_xcc_upgrade() {
        const DEPOSIT_AMOUNT: u128 = 1;

        // Deploy v3.3.1 Engine with the XCC router contract it had at the time.
        let v1_bytes = std::fs::read("src/tests/res/xcc_router_v1.wasm").unwrap();
        let XccTestContext {
            aurora,
            mut signer,
            signer_address,
            chain_id,
            wnear_account,
        } = inner_init_xcc(v1_bytes, true).await.unwrap();

        let router_account_id = create_router_account_id(&signer_address, &aurora);

        // Do XCC interaction to create router account
        let promise = PromiseCreateArgs {
            target_account_id: wnear_account.id(),
            method: "near_deposit".into(),
            args: b"{}".to_vec(),
            attached_balance: Yocto::new(1),
            attached_gas: NearGas::new(5_000_000_000_000),
        };
        let promise_args = PromiseArgs::Create(promise);
        let xcc_args = CrossContractCallArgs::Eager(promise_args);
        submit_xcc_transaction(&xcc_args, &aurora, &mut signer, chain_id)
            .await
            .unwrap();
        assert_eq!(
            nep_141_balance_of(&wnear_account, &router_account_id).await,
            DEPOSIT_AMOUNT,
        );

        // Upgrade to latest engine code
        aurora
            .stage_upgrade(utils::AuroraRunner::get_engine_code())
            .max_gas()
            .transact()
            .await
            .unwrap();
        aurora.deploy_upgrade().max_gas().transact().await.unwrap();

        // Upgrade to Engine to have latest XCC
        let current_xcc_version = super::router_version();
        aurora
            .factory_update(super::contract_bytes())
            .max_gas()
            .transact()
            .await
            .unwrap();

        // Confirm that XCC v1 router account still works
        submit_xcc_transaction(&xcc_args, &aurora, &mut signer, chain_id)
            .await
            .unwrap();
        assert_eq!(
            nep_141_balance_of(&wnear_account, &router_account_id).await,
            2 * DEPOSIT_AMOUNT,
        );

        // Create new account on Aurora
        let mut v2_signer = utils::Signer::random();
        let v2_signer_address = utils::address_from_secret_key(&v2_signer.secret_key);
        let wnear_address = aurora.factory_get_wnear_address().await.unwrap().result;
        let wnear_erc20 = {
            let constructor = ERC20Constructor::load();
            let contract = constructor.0.deployed_at(wnear_address);
            ERC20(contract)
        };
        transfer_nep_141_to_erc_20(
            &wnear_account,
            &wnear_erc20,
            &aurora.root(),
            v2_signer_address,
            WNEAR_AMOUNT.as_yoctonear(),
            &aurora,
        )
        .await
        .unwrap();
        approve_xcc_precompile(&wnear_erc20, &aurora, chain_id, &mut v2_signer)
            .await
            .unwrap();

        // Use XCC to create account with v2 router contract
        let v2_router_account_id = create_router_account_id(&v2_signer_address, &aurora);
        submit_xcc_transaction(&xcc_args, &aurora, &mut v2_signer, chain_id)
            .await
            .unwrap();
        assert_eq!(
            nep_141_balance_of(&wnear_account, &v2_router_account_id).await,
            DEPOSIT_AMOUNT,
        );
        assert_eq!(
            get_xcc_router_version(&aurora, &v2_router_account_id).await,
            current_xcc_version,
        );

        // Upgrade to Engine to have fake XCC v3
        super::change_router_version(current_xcc_version + 1);
        aurora
            .factory_update(super::contract_bytes())
            .max_gas()
            .transact()
            .await
            .unwrap();

        // Use v2 XCC router account and confirm it is upgraded to v3
        submit_xcc_transaction(&xcc_args, &aurora, &mut v2_signer, chain_id)
            .await
            .unwrap();
        assert_eq!(
            nep_141_balance_of(&wnear_account, &v2_router_account_id).await,
            2 * DEPOSIT_AMOUNT,
        );
        assert_eq!(
            get_xcc_router_version(&aurora, &v2_router_account_id).await,
            current_xcc_version + 1,
        );

        // Upgrade Engine to have fake XCC v4
        super::change_router_version(current_xcc_version + 2);
        aurora
            .factory_update(super::contract_bytes())
            .max_gas()
            .transact()
            .await
            .unwrap();

        // Use XCC direct funding and confirm upgrade still happens
        aurora
            .fund_xcc_sub_account(v2_signer_address, Some(wnear_account.id()))
            .max_gas()
            .transact()
            .await
            .unwrap();
        assert_eq!(
            get_xcc_router_version(&aurora, &v2_router_account_id).await,
            current_xcc_version + 2,
        );

        // Restore XCC router version to not leave the git repo dirty
        super::change_router_version(current_xcc_version);
    }

    #[allow(clippy::too_many_lines, clippy::future_not_send)]
    async fn test_xcc_precompile_common(is_scheduled: bool) {
        let XccTestContext {
            aurora,
            mut signer,
            signer_address,
            chain_id,
            wnear_account,
        } = init_xcc().await.unwrap();

        let router_account = format!(
            "{}.{}",
            hex::encode(signer_address.as_bytes()),
            aurora.id().as_ref()
        );
        let router_account_id = router_account.parse().unwrap();

        // 1. Deploy NEP-141 token.
        let ft_owner = create_sub_account(&aurora.root(), "ft_owner", STORAGE_AMOUNT)
            .await
            .unwrap();
        let token = create_sub_account(&aurora.root(), "test_token", STORAGE_AMOUNT)
            .await
            .unwrap();
        let nep_141_supply = 500;
        let nep_141 = deploy_nep_141(&token, &ft_owner, nep_141_supply, &aurora)
            .await
            .unwrap();

        // 2. Register EVM router contract
        let result = aurora
            .root()
            .call(&nep_141.id(), "storage_deposit")
            .args_json(json!({
                "account_id": router_account,
            }))
            .deposit(STORAGE_AMOUNT)
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());

        // 3. Give router some tokens
        let transfer_amount: u128 = 199;
        let result = ft_owner
            .call(&nep_141.id(), "ft_transfer")
            .args_json(json!({
                "receiver_id": router_account,
                "amount": format!("{transfer_amount}"),
            }))
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());
        assert_eq!(
            nep_141_balance_of(&nep_141, &ft_owner.id()).await,
            nep_141_supply - transfer_amount
        );

        // 4. Use xcc precompile to send those tokens back
        let args = json!({
            "receiver_id": ft_owner.id().as_ref(),
            "amount": format!("{transfer_amount}"),
        })
        .to_string();
        let promise = PromiseCreateArgs {
            target_account_id: nep_141.id(),
            method: "ft_transfer".into(),
            args: args.into_bytes(),
            attached_balance: Yocto::new(1),
            attached_gas: NearGas::new(100_000_000_000_000),
        };
        let callback = PromiseCreateArgs {
            target_account_id: nep_141.id(),
            method: "ft_balance_of".into(),
            args: format!(r#"{{"account_id":"{router_account}"}}"#).into_bytes(),
            attached_balance: Yocto::new(0),
            attached_gas: NearGas::new(2_000_000_000_000),
        };
        let promise_args = PromiseArgs::Callback(PromiseWithCallbackArgs {
            base: promise,
            callback,
        });
        let xcc_args = if is_scheduled {
            CrossContractCallArgs::Delayed(promise_args)
        } else {
            CrossContractCallArgs::Eager(promise_args)
        };
        let engine_balance_before_xcc = get_engine_near_balance(&aurora).await;
        let result = submit_xcc_transaction(&xcc_args, &aurora, &mut signer, chain_id).await;
        assert!(result.is_ok(), "{:?}", result.err());

        let engine_balance_after_xcc = get_engine_near_balance(&aurora).await;
        assert!(
            // engine loses less than 0.01 NEAR
            engine_balance_after_xcc.max(engine_balance_before_xcc)
                - engine_balance_after_xcc.min(engine_balance_before_xcc)
                < 10_000_000_000_000_000_000_000,
            "Engine lost too much NEAR funding xcc: Before={:?} After={:?} Eq={:?}",
            engine_balance_before_xcc,
            engine_balance_after_xcc,
            engine_balance_after_xcc.max(engine_balance_before_xcc)
                - engine_balance_after_xcc.min(engine_balance_before_xcc)
        );

        let router_balance = aurora.node.get_balance(&router_account_id).await.unwrap();
        assert!(
            // router loses less than 0.01 NEAR from its allocated funds
            aurora_engine_precompiles::xcc::state::STORAGE_AMOUNT.as_u128() - router_balance
                < 10_000_000_000_000_000_000_000,
            "Router lost too much NEAR: Balance={router_balance}",
        );
        // Router has no wNEAR balance because it all was unwrapped to actual NEAR
        assert_eq!(
            nep_141_balance_of(&wnear_account, &router_account_id).await,
            0,
        );

        if is_scheduled {
            // The promise was only scheduled, not executed immediately. So the FT balance has not changed yet.
            assert_eq!(
                nep_141_balance_of(&nep_141, &ft_owner.id()).await,
                nep_141_supply - transfer_amount
            );

            // Now we execute the scheduled promise
            let result = aurora
                .root()
                .call(&router_account_id, "execute_scheduled")
                .args_json(json!({
                    "nonce": "0"
                }))
                .max_gas()
                .transact()
                .await
                .unwrap();
            assert!(result.is_success(), "{result:?}");
        }

        assert_eq!(
            nep_141_balance_of(&nep_141, &ft_owner.id()).await,
            nep_141_supply
        );
    }

    /// Default XCC initialization (latest Aurora Engine code + latest XCC router code).
    async fn init_xcc() -> anyhow::Result<XccTestContext> {
        inner_init_xcc(super::contract_bytes(), false).await
    }

    /// Deploys the EVM, sets xcc router code, deploys wnear contract, bridges wnear into EVM,
    /// and calls `factory_set_wnear_address`
    async fn inner_init_xcc(
        xcc_wasm_bytes: Vec<u8>,
        use_v331: bool,
    ) -> anyhow::Result<XccTestContext> {
        let aurora = if use_v331 {
            deploy_engine_v331().await
        } else {
            deploy_engine().await
        };
        let chain_id = aurora.get_chain_id().await?.result.as_u64();
        let result = aurora.factory_update(xcc_wasm_bytes).transact().await?;
        assert!(result.is_success());

        let mut signer = utils::Signer::random();
        let signer_address = utils::address_from_secret_key(&signer.secret_key);

        // Setup wNEAR contract and bridge it to Aurora
        let wnear_contract = deploy_wnear(&aurora).await?;
        let wnear_erc20 = deploy_erc20_from_nep_141(wnear_contract.id().as_ref(), &aurora).await?;

        transfer_nep_141_to_erc_20(
            &wnear_contract,
            &wnear_erc20,
            &aurora.root(),
            signer_address,
            WNEAR_AMOUNT.as_yoctonear(),
            &aurora,
        )
        .await
        .unwrap();
        let result = aurora
            .factory_set_wnear_address(wnear_erc20.0.address)
            .transact()
            .await?;
        assert!(result.is_success());

        let wnear_address = aurora.factory_get_wnear_address().await.unwrap().result;
        assert_eq!(wnear_address, wnear_erc20.0.address);

        approve_xcc_precompile(&wnear_erc20, &aurora, chain_id, &mut signer).await?;

        Ok(XccTestContext {
            aurora,
            signer,
            signer_address,
            chain_id,
            wnear_account: wnear_contract,
        })
    }

    struct XccTestContext {
        pub aurora: EngineContract,
        pub signer: utils::Signer,
        pub signer_address: Address,
        pub chain_id: u64,
        pub wnear_account: RawContract,
    }

    fn create_router_account_id(signer_address: &Address, aurora: &EngineContract) -> AccountId {
        let router_account = format!(
            "{}.{}",
            hex::encode(signer_address.as_bytes()),
            aurora.id().as_ref()
        );
        router_account.parse().unwrap()
    }

    /// The signer approves the XCC precompile to spend its wrapped NEAR
    async fn approve_xcc_precompile(
        wnear_erc20: &ERC20,
        aurora: &EngineContract,
        chain_id: u64,
        signer: &mut utils::Signer,
    ) -> anyhow::Result<()> {
        let approve_tx = wnear_erc20.approve(
            cross_contract_call::ADDRESS,
            WNEAR_AMOUNT.as_yoctonear().into(),
            signer.use_nonce().into(),
        );
        let signed_transaction =
            utils::sign_transaction(approve_tx, Some(chain_id), &signer.secret_key);
        let result = aurora
            .submit(rlp::encode(&signed_transaction).to_vec())
            .transact()
            .await?;
        if !result.is_success() {
            return Err(anyhow::Error::msg("Failed Approve transaction"));
        };
        Ok(())
    }

    async fn submit_xcc_transaction(
        xcc_args: &CrossContractCallArgs,
        aurora: &EngineContract,
        signer: &mut utils::Signer,
        chain_id: u64,
    ) -> anyhow::Result<()> {
        let transaction = TransactionLegacy {
            nonce: signer.use_nonce().into(),
            gas_price: 0u64.into(),
            gas_limit: u64::MAX.into(),
            to: Some(cross_contract_call::ADDRESS),
            value: Wei::zero(),
            data: borsh::to_vec(&xcc_args).unwrap(),
        };
        let signed_transaction =
            utils::sign_transaction(transaction, Some(chain_id), &signer.secret_key);
        let result = aurora
            .submit(rlp::encode(&signed_transaction).to_vec())
            .max_gas()
            .transact()
            .await?;

        match &result.value().status {
            TransactionStatus::Succeed(_) => Ok(()),
            TransactionStatus::Revert(b) => {
                let revert_message = ethabi::decode(&[ethabi::ParamType::String], &b[4..])
                    .unwrap()
                    .pop()
                    .unwrap()
                    .into_string()
                    .unwrap();
                anyhow::bail!("TX has been reverted with message: {revert_message}");
            }
            _ => anyhow::bail!("Wrong status of the transaction"),
        }
    }

    async fn get_engine_near_balance(aurora: &EngineContract) -> u128 {
        nep_141_balance_of(aurora.as_raw_contract(), &aurora.id()).await
    }

    pub async fn deploy_wnear(aurora: &EngineContract) -> anyhow::Result<RawContract> {
        let contract_bytes = std::fs::read("src/tests/res/w_near.wasm").unwrap();
        let wrap_account = create_sub_account(&aurora.root(), "wrap", STORAGE_AMOUNT).await?;
        let contract = wrap_account.deploy(&contract_bytes).await?;

        let result = aurora.root().call(&contract.id(), "new").transact().await?;
        assert!(result.is_success(), "{result:?}");

        // Need to register Aurora contract so that it can receive tokens
        let result = aurora
            .root()
            .call(&wrap_account.id(), "storage_deposit")
            .args_json(json!({"account_id": aurora.id().as_ref()}))
            .deposit(STORAGE_AMOUNT)
            .transact()
            .await?;
        assert!(result.is_success(), "{result:?}");

        // Also need to register root account
        let result = aurora
            .root()
            .call(&wrap_account.id(), "storage_deposit")
            .args_json(json!({"account_id": aurora.root().id().as_ref()}))
            .deposit(STORAGE_AMOUNT)
            .transact()
            .await?;
        assert!(result.is_success(), "{result:?}");

        // Mint some wNEAR for the root account to use
        let result = aurora
            .root()
            .call(&wrap_account.id(), "near_deposit")
            .deposit(WNEAR_AMOUNT.saturating_mul(3))
            .transact()
            .await?;
        assert!(result.is_success(), "{result:?}");

        Ok(contract)
    }

    async fn deploy_fibonacci(aurora: &EngineContract) -> anyhow::Result<AccountId> {
        let fib_contract_bytes = {
            let base_path = Path::new("..").join("etc").join("tests").join("fibonacci");
            let output_path =
                base_path.join("target/wasm32-unknown-unknown/release/fibonacci_on_near.wasm");
            utils::rust::compile(base_path);
            std::fs::read(output_path)?
        };
        let fib_account =
            create_sub_account(&aurora.root(), "fib", NearToken::from_near(50)).await?;
        fib_account
            .deploy(&fib_contract_bytes)
            .await
            .map(|contract| contract.id())
    }
}

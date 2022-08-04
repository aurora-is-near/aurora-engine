use crate::test_utils::{self, AuroraRunner};
use crate::tests::erc20_connector::sim_tests;
use crate::tests::state_migration::deploy_evm;
use aurora_engine_precompiles::xcc::{costs, cross_contract_call};
use aurora_engine_transactions::legacy::TransactionLegacy;
use aurora_engine_types::parameters::{
    CrossContractCallArgs, PromiseArgs, PromiseCreateArgs, PromiseWithCallbackArgs,
};
use aurora_engine_types::types::{Address, EthGas, NearGas, Wei, Yocto};
use aurora_engine_types::U256;
use borsh::BorshSerialize;
use near_primitives::transaction::Action;
use near_primitives_core::contract::ContractCode;
use std::fs;
use std::path::Path;

#[test]
fn test_xcc_eth_gas_cost() {
    let mut runner = test_utils::deploy_evm();
    runner.standalone_runner = None;
    let xcc_wasm_bytes = contract_bytes();
    let _ = runner.call("factory_update", "aurora", xcc_wasm_bytes);
    let mut signer = test_utils::Signer::random();
    runner.context.block_index = aurora_engine::engine::ZERO_ADDRESS_FIX_HEIGHT + 1;

    // Baseline transaction that does essentially nothing.
    let (_, baseline) = runner
        .submit_with_signer_profiled(&mut signer, |nonce| TransactionLegacy {
            nonce,
            gas_price: U256::zero(),
            gas_limit: u64::MAX.into(),
            to: Some(Address::from_array([0; 20])),
            value: Wei::zero(),
            data: Vec::new(),
        })
        .unwrap();

    let mut profile_for_promise = |p: PromiseArgs| -> (u64, u64) {
        let data = CrossContractCallArgs::Eager(p).try_to_vec().unwrap();
        let input_length = data.len();
        let (_, profile) = runner
            .submit_with_signer_profiled(&mut signer, |nonce| TransactionLegacy {
                nonce,
                gas_price: U256::zero(),
                gas_limit: u64::MAX.into(),
                to: Some(cross_contract_call::ADDRESS),
                value: Wei::zero(),
                data,
            })
            .unwrap();
        // Subtract off baseline transaction to isolate just precompile things
        (
            u64::try_from(input_length).unwrap(),
            profile.all_gas() - baseline.all_gas(),
        )
    };

    let promise = PromiseCreateArgs {
        target_account_id: "some_account.near".parse().unwrap(),
        method: "some_method".into(),
        args: b"hello_world".to_vec(),
        attached_balance: Yocto::new(56),
        attached_gas: NearGas::new(0),
    };
    // Shorter input
    let (x1, y1) = profile_for_promise(PromiseArgs::Create(promise.clone()));
    // longer input
    let (x2, y2) = profile_for_promise(PromiseArgs::Callback(PromiseWithCallbackArgs {
        base: promise.clone(),
        callback: promise,
    }));

    // NEAR costs (inferred from a line through (x1, y1) and (x2, y2))
    let xcc_cost_per_byte = (y2 - y1) / (x2 - x1);
    let xcc_base_cost = NearGas::new(y1 - xcc_cost_per_byte * x1);

    // Convert to EVM cost using conversion ratio
    let xcc_base_cost = EthGas::new(xcc_base_cost.as_u64() / costs::CROSS_CONTRACT_CALL_NEAR_GAS);
    let xcc_cost_per_byte = xcc_cost_per_byte / costs::CROSS_CONTRACT_CALL_NEAR_GAS;

    let within_5_percent = |a: u64, b: u64| -> bool {
        let x = a.max(b);
        let y = a.min(b);

        20 * (x - y) <= x
    };
    assert!(
        within_5_percent(
            xcc_base_cost.as_u64(),
            costs::CROSS_CONTRACT_CALL_BASE.as_u64()
        ),
        "Incorrect xcc base cost. Expected: {} Actual: {}",
        xcc_base_cost,
        costs::CROSS_CONTRACT_CALL_BASE
    );

    assert!(
        within_5_percent(xcc_cost_per_byte, costs::CROSS_CONTRACT_CALL_BYTE.as_u64()),
        "Incorrect xcc per byte cost. Expected: {} Actual: {}",
        xcc_cost_per_byte,
        costs::CROSS_CONTRACT_CALL_BYTE
    );
}

#[test]
fn test_xcc_precompile_eager() {
    test_xcc_precompile_common(false)
}

#[test]
fn test_xcc_precompile_scheduled() {
    test_xcc_precompile_common(true)
}

fn test_xcc_precompile_common(is_scheduled: bool) {
    let aurora = deploy_evm();
    let xcc_wasm_bytes = contract_bytes();
    aurora
        .user
        .call(
            aurora.contract.account_id(),
            "factory_update",
            &xcc_wasm_bytes,
            near_sdk_sim::DEFAULT_GAS,
            0,
        )
        .assert_success();

    let mut signer = test_utils::Signer::random();
    let signer_address = test_utils::address_from_secret_key(&signer.secret_key);
    let router_account = format!(
        "{}.{}",
        hex::encode(signer_address.as_bytes()),
        aurora.contract.account_id.as_str()
    );

    // 1. Deploy NEP-141 token.
    let ft_owner = aurora.user.create_user(
        "ft_owner.root".parse().unwrap(),
        near_sdk_sim::STORAGE_AMOUNT,
    );
    let nep_141_supply = 500;
    let nep_141_token = sim_tests::deploy_nep_141(
        "test_token.root",
        ft_owner.account_id.as_ref(),
        nep_141_supply,
        &aurora,
    );

    // 2. Register EVM router contract
    let args = serde_json::json!({
        "account_id": router_account,
    })
    .to_string();
    aurora
        .user
        .call(
            nep_141_token.account_id(),
            "storage_deposit",
            args.as_bytes(),
            near_sdk_sim::DEFAULT_GAS,
            near_sdk_sim::STORAGE_AMOUNT,
        )
        .assert_success();

    // 3. Give router some tokens
    let transfer_amount: u128 = 199;
    let args = serde_json::json!({
        "receiver_id": router_account,
        "amount": format!("{}", transfer_amount),
    })
    .to_string();
    ft_owner
        .call(
            nep_141_token.account_id(),
            "ft_transfer",
            args.as_bytes(),
            near_sdk_sim::DEFAULT_GAS,
            1,
        )
        .assert_success();
    assert_eq!(
        sim_tests::nep_141_balance_of(ft_owner.account_id.as_str(), &nep_141_token, &aurora),
        nep_141_supply - transfer_amount
    );

    // 4. Use xcc precompile to send those tokens back
    let args = serde_json::json!({
        "receiver_id": ft_owner.account_id.as_str(),
        "amount": format!("{}", transfer_amount),
    })
    .to_string();
    let promise = PromiseCreateArgs {
        target_account_id: nep_141_token.account_id.as_str().parse().unwrap(),
        method: "ft_transfer".into(),
        args: args.into_bytes(),
        attached_balance: Yocto::new(1),
        attached_gas: NearGas::new(100_000_000_000_000),
    };
    let xcc_args = if is_scheduled {
        CrossContractCallArgs::Delayed(PromiseArgs::Create(promise))
    } else {
        CrossContractCallArgs::Eager(PromiseArgs::Create(promise))
    };
    let transaction = TransactionLegacy {
        nonce: signer.use_nonce().into(),
        gas_price: 0u64.into(),
        gas_limit: u64::MAX.into(),
        to: Some(cross_contract_call::ADDRESS),
        value: Wei::zero(),
        data: xcc_args.try_to_vec().unwrap(),
    };
    let signed_transaction = test_utils::sign_transaction(
        transaction,
        Some(AuroraRunner::default().chain_id),
        &signer.secret_key,
    );
    aurora
        .user
        .call(
            aurora.contract.account_id(),
            "submit",
            &rlp::encode(&signed_transaction),
            near_sdk_sim::DEFAULT_GAS,
            0,
        )
        .assert_success();

    let rt = aurora.user.borrow_runtime();
    for id in rt.last_outcomes.iter() {
        println!("{:?}\n\n", rt.outcome(id).unwrap());
    }
    drop(rt);

    if is_scheduled {
        // The promise was only scheduled, not executed immediately. So the FT balance has not changed yet.
        assert_eq!(
            sim_tests::nep_141_balance_of(ft_owner.account_id.as_str(), &nep_141_token, &aurora),
            nep_141_supply - transfer_amount
        );

        // Now we execute the scheduled promise
        aurora
            .user
            .call(
                router_account.parse().unwrap(),
                "execute_scheduled",
                b"{\"nonce\": \"0\"}",
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .assert_success();
    }

    assert_eq!(
        sim_tests::nep_141_balance_of(ft_owner.account_id.as_str(), &nep_141_token, &aurora),
        nep_141_supply
    );
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

    let (maybe_outcome, maybe_error) = router.call(
        "schedule",
        "aurora",
        PromiseArgs::Create(promise.clone()).try_to_vec().unwrap(),
    );
    assert!(maybe_error.is_none());
    let outcome = maybe_outcome.unwrap();
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

    let promise = PromiseCreateArgs {
        target_account_id: "some_account.near".parse().unwrap(),
        method: "some_method".into(),
        args: b"hello_world".to_vec(),
        attached_balance: Yocto::new(56),
        attached_gas: NearGas::new(100_000_000_000_000),
    };

    let (maybe_outcome, maybe_error) = router.call(
        "execute",
        "aurora",
        PromiseArgs::Create(promise.clone()).try_to_vec().unwrap(),
    );
    assert!(maybe_error.is_none());
    let outcome = maybe_outcome.unwrap();

    assert!(
        outcome.burnt_gas < costs::ROUTER_EXEC.as_u64(),
        "{:?} not less than {:?}",
        outcome.burnt_gas,
        costs::ROUTER_EXEC
    );
    assert_eq!(outcome.action_receipts.len(), 1);
    assert_eq!(
        outcome.action_receipts[0].0.as_str(),
        promise.target_account_id.as_ref()
    );
    let receipt = &outcome.action_receipts[0].1;
    assert_eq!(receipt.actions.len(), 1);
    let action = &receipt.actions[0];
    match action {
        Action::FunctionCall(function_call) => {
            assert_eq!(function_call.method_name, promise.method);
            assert_eq!(function_call.args, promise.args);
            assert_eq!(function_call.deposit, promise.attached_balance.as_u128());
            assert_eq!(function_call.gas, promise.attached_gas.as_u64());
        }
        other => panic!("Unexpected action {:?}", other),
    };
}

fn deploy_router() -> AuroraRunner {
    let mut router = AuroraRunner::default();
    router.code = ContractCode::new(contract_bytes(), None);

    router.context.current_account_id = "some_address.aurora".parse().unwrap();
    router.context.predecessor_account_id = "aurora".parse().unwrap();

    let (maybe_outcome, maybe_error) = router.call("initialize", "aurora", Vec::new());
    assert!(maybe_error.is_none());
    let outcome = maybe_outcome.unwrap();
    assert!(outcome.used_gas < aurora_engine::xcc::INITIALIZE_GAS.as_u64());

    router
}

fn contract_bytes() -> Vec<u8> {
    let base_path = Path::new("../etc").join("xcc-router");
    let output_path = base_path.join("target/wasm32-unknown-unknown/release/xcc_router.wasm");
    test_utils::rust::compile(base_path);
    fs::read(output_path).unwrap()
}

use crate::prelude::{H160, H256};
use crate::utils::solidity::erc20::{ERC20Constructor, ERC20};
use crate::utils::{self, standalone, Signer};
use aurora_engine_modexp::AuroraModExp;
use aurora_engine_types::parameters::engine::TransactionStatus;
use aurora_engine_types::{
    parameters::{CrossContractCallArgs, PromiseArgs, PromiseCreateArgs},
    storage,
    types::{Address, NearGas, Wei, Yocto},
    U256,
};
use engine_standalone_storage::sync;
use engine_standalone_tracing::{
    sputnik,
    types::call_tracer::{self, CallTracer},
};

#[test]
fn test_trace_contract_deploy() {
    let mut runner = standalone::StandaloneRunner::default();
    let mut signer = Signer::random();

    runner.init_evm();

    let constructor = ERC20Constructor::load();
    let deploy_tx = constructor.deploy("Test", "TST", signer.use_nonce().into());
    let (deploy_result, call_tracer) = runner
        .submit_transaction_with_call_stack_tracing(&signer.secret_key, deploy_tx)
        .unwrap();
    let mut call_tracer = call_tracer.unwrap();
    let contract_address = {
        let bytes = utils::unwrap_success_slice(&deploy_result);
        Address::try_from_slice(bytes).unwrap()
    };
    let code = runner.get_code(&contract_address);

    assert_eq!(call_tracer.call_stack.len(), 1);
    let trace = call_tracer.call_stack.pop().unwrap();

    assert_eq!(trace.to, Some(contract_address));
    assert_eq!(trace.output, code);
}

#[test]
fn test_trace_precompile_direct_call() {
    let mut runner = standalone::StandaloneRunner::default();
    let mut signer = Signer::random();

    runner.init_evm();

    let input = hex::decode("0000ca110000").unwrap();
    let precompile_cost = {
        use aurora_engine_precompiles::Precompile;
        let context = aurora_evm::Context {
            address: H160::default(),
            caller: H160::default(),
            apparent_value: U256::zero(),
        };
        let result =
            aurora_engine_precompiles::identity::Identity.run(&input, None, &context, false);
        result.unwrap().cost.as_u64()
    };
    let tx = aurora_engine_transactions::legacy::TransactionLegacy {
        nonce: signer.use_nonce().into(),
        gas_price: U256::zero(),
        gas_limit: u64::MAX.into(),
        to: Some(aurora_engine_precompiles::identity::Identity::ADDRESS),
        value: Wei::zero(),
        data: input.clone(),
    };
    let intrinsic_cost = {
        let signed_tx =
            utils::sign_transaction(tx.clone(), Some(runner.chain_id), &signer.secret_key);
        let kind = aurora_engine_transactions::EthTransactionKind::Legacy(signed_tx);
        let norm_tx = aurora_engine_transactions::NormalizedEthTransaction::try_from(kind).unwrap();
        norm_tx
            .intrinsic_gas(&aurora_evm::Config::shanghai())
            .unwrap()
    };

    let (standalone_result, call_tracer) = runner
        .submit_transaction_with_call_stack_tracing(&signer.secret_key, tx)
        .unwrap();
    let mut call_tracer = call_tracer.unwrap();
    assert!(standalone_result.status.is_ok());
    assert_eq!(call_tracer.call_stack.len(), 1);

    let trace = call_tracer.call_stack.pop().unwrap();

    let expected_trace = call_tracer::CallFrame {
        call_type: call_tracer::CallType::Call,
        from: utils::address_from_secret_key(&signer.secret_key),
        to: Some(aurora_engine_precompiles::identity::Identity::ADDRESS),
        value: U256::zero(),
        gas: u64::MAX,
        gas_used: intrinsic_cost + precompile_cost,
        input: input.clone(),
        output: input,
        error: None,
        calls: Vec::new(),
    };

    assert_eq!(trace, expected_trace);

    runner.close();
}

#[test]
fn test_trace_contract_single_call() {
    let mut runner = standalone::StandaloneRunner::default();
    let mut signer = Signer::random();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);

    runner.init_evm();

    let constructor = ERC20Constructor::load();
    let deploy_tx = constructor.deploy("Test", "TST", signer.use_nonce().into());
    let deploy_result = runner
        .submit_transaction(&signer.secret_key, deploy_tx)
        .unwrap();
    let contract_address = {
        let bytes = utils::unwrap_success_slice(&deploy_result);
        Address::try_from_slice(bytes).unwrap()
    };
    let contract = ERC20(constructor.0.deployed_at(contract_address));

    let tx = contract.balance_of(signer_address, signer.use_nonce().into());
    let (standalone_result, call_tracer) = runner
        .submit_transaction_with_call_stack_tracing(&signer.secret_key, tx.clone())
        .unwrap();
    let mut call_tracer = call_tracer.unwrap();
    assert!(standalone_result.status.is_ok());
    assert_eq!(call_tracer.call_stack.len(), 1);

    let trace = call_tracer.call_stack.pop().unwrap();

    let expected_trace = call_tracer::CallFrame {
        call_type: call_tracer::CallType::Call,
        from: signer_address,
        to: Some(contract_address),
        value: U256::zero(),
        gas: u64::MAX,
        gas_used: trace.gas_used,
        input: tx.data,
        output: [0u8; 32].to_vec(),
        error: None,
        calls: Vec::new(),
    };

    assert_eq!(trace, expected_trace);

    runner.close();
}

#[test]
fn test_trace_contract_with_sub_call() {
    use crate::tests::uniswap::UniswapTestContext;
    const MINT_AMOUNT: u64 = 1_000_000_000_000;
    const LIQUIDITY_AMOUNT: u64 = MINT_AMOUNT / 5;
    const OUTPUT_AMOUNT: u64 = LIQUIDITY_AMOUNT / 100;

    let mut context = UniswapTestContext::new("uniswap");
    let (token_a, token_b) = context.create_token_pair(MINT_AMOUNT.into());
    let pool = context.create_pool(&token_a, &token_b);

    let (_result, _profile) =
        context.add_equal_liquidity(LIQUIDITY_AMOUNT.into(), &token_a, &token_b);

    context.approve_erc20(&token_a, context.swap_router.0.address, U256::MAX);
    context.approve_erc20(&token_b, context.swap_router.0.address, U256::MAX);
    let params =
        UniswapTestContext::exact_output_single_params(OUTPUT_AMOUNT.into(), &token_a, &token_b);

    let mut listener = CallTracer::default();
    let (_amount_in, _profile) = sputnik::traced_call(&mut listener, || {
        context
            .runner
            .submit_with_signer_profiled(&mut context.signer, |nonce| {
                context.swap_router.exact_output_single(&params, nonce)
            })
            .unwrap()
    });

    assert_eq!(listener.call_stack.len(), 1);

    let user_address = utils::address_from_secret_key(&context.signer.secret_key);
    let router_address = context.swap_router.0.address;
    let pool_address = pool.0.address;
    let b_address = token_b.0.address;
    let a_address = token_a.0.address;

    // Call flow:
    // User -> Router.exactOutputSingle -> Pool.swap -> B.transfer
    //                                               -> A.balanceOf
    //                                               -> Router.uniswapV3SwapCallback -> A.transferFrom
    //                                               -> A.balanceOf
    let root_call = listener.call_stack.first().unwrap();
    assert_eq!(root_call.from, user_address);
    assert_eq!(root_call.to.unwrap(), router_address);

    let call = subcall_lense(root_call, &[0]);
    assert_eq!(call.from, router_address);
    assert_eq!(call.to.unwrap(), pool_address);

    let call = subcall_lense(root_call, &[0, 0]);
    assert_eq!(call.from, pool_address);
    assert_eq!(call.to.unwrap(), b_address);

    let call = subcall_lense(root_call, &[0, 1]);
    assert_eq!(call.from, pool_address);
    assert_eq!(call.to.unwrap(), a_address);

    let call = subcall_lense(root_call, &[0, 2]);
    assert_eq!(call.from, pool_address);
    assert_eq!(call.to.unwrap(), router_address);

    let call = subcall_lense(root_call, &[0, 2, 0]);
    assert_eq!(call.from, router_address);
    assert_eq!(call.to.unwrap(), a_address);

    let call = subcall_lense(root_call, &[0, 3]);
    assert_eq!(call.from, pool_address);
    assert_eq!(call.to.unwrap(), a_address);
}

#[test]
fn test_trace_contract_with_precompile_sub_call() {
    let mut runner = standalone::StandaloneRunner::default();
    let mut signer = Signer::random();

    runner.init_evm();

    let constructor = utils::solidity::standard_precompiles::PrecompilesConstructor::load();
    let deploy_tx = constructor.deploy(signer.use_nonce().into());
    let deploy_result = runner
        .submit_transaction(&signer.secret_key, deploy_tx)
        .unwrap();
    let contract_address = {
        let bytes = utils::unwrap_success_slice(&deploy_result);
        Address::try_from_slice(bytes).unwrap()
    };
    let contract = utils::solidity::standard_precompiles::PrecompilesContract(
        constructor.0.deployed_at(contract_address),
    );

    // This transaction calls the standard precompiles (`ecrecover`, `sha256`, etc) one aft the other.
    // So the trace is one top-level call with multiple sub-calls (and the sub-calls contain no further sub-calls).
    let tx = contract.call_method("test_all", signer.use_nonce().into());
    let (standalone_result, call_tracer) = runner
        .submit_transaction_with_call_stack_tracing(&signer.secret_key, tx.clone())
        .unwrap();
    let mut call_tracer = call_tracer.unwrap();
    assert!(standalone_result.status.is_ok());
    assert_eq!(call_tracer.call_stack.len(), 1);

    let trace = call_tracer.call_stack.pop().unwrap();
    assert_eq!(trace.calls.len(), 8);
    for call in trace.calls {
        assert!(call.calls.is_empty());
    }

    runner.close();
}

#[test]
fn test_contract_create_too_large() {
    let mut runner = standalone::StandaloneRunner::default();
    let signer = Signer::random();

    runner.init_evm();

    let tx_data = {
        let tx_data_hex =
            std::fs::read_to_string("src/tests/res/contract_data_too_large.hex").unwrap();
        hex::decode(
            tx_data_hex
                .strip_prefix("0x")
                .unwrap_or(&tx_data_hex)
                .trim(),
        )
        .unwrap()
    };
    let tx = aurora_engine_transactions::legacy::TransactionLegacy {
        nonce: U256::zero(),
        gas_price: U256::zero(),
        gas_limit: u64::MAX.into(),
        to: None,
        value: Wei::zero(),
        data: tx_data,
    };

    let (standalone_result, call_tracer) = runner
        .submit_transaction_with_call_stack_tracing(&signer.secret_key, tx)
        .unwrap();
    let _call_tracer = call_tracer.unwrap();
    assert!(matches!(
        standalone_result.status,
        TransactionStatus::CreateContractLimit
    ));
}

#[allow(clippy::too_many_lines)]
#[test]
fn test_trace_precompiles_with_subcalls() {
    // The XCC precompile does internal sub-calls. We will trace an XCC call.

    let mut runner = standalone::StandaloneRunner::default();
    let mut signer = Signer::random();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let xcc_address = aurora_engine_precompiles::xcc::cross_contract_call::ADDRESS;

    runner.init_evm();

    // Deploy an ERC-20 contract to act as wNEAR. It doesn't actually need to be bridged for
    // this test because we are not executing any scheduled promises.
    let constructor = ERC20Constructor::load();
    let deploy_tx = constructor.deploy("wNEAR", "WNEAR", signer.use_nonce().into());
    let deploy_result = runner
        .submit_transaction(&signer.secret_key, deploy_tx)
        .unwrap();
    let wnear_address = {
        let bytes = utils::unwrap_success_slice(&deploy_result);
        Address::try_from_slice(bytes).unwrap()
    };
    let wnear = ERC20(constructor.0.deployed_at(wnear_address));
    let mint_tx = wnear.mint(signer_address, u128::MAX.into(), signer.use_nonce().into());
    runner
        .submit_transaction(&signer.secret_key, mint_tx)
        .unwrap();
    let approve_tx = wnear.approve(xcc_address, U256::MAX, signer.use_nonce().into());
    runner
        .submit_transaction(&signer.secret_key, approve_tx)
        .unwrap();
    // Ensure the above ERC-20 token is registered as if it were a bridged token
    {
        runner.env.block_height += 1;
        let storage = &mut runner.storage;
        let env = &runner.env;

        let tx_kind = sync::types::TransactionKind::deploy_erc20(
            &aurora_engine::parameters::DeployErc20TokenArgs::Legacy("wrap.near".parse().unwrap()),
        );
        let mut tx =
            standalone::StandaloneRunner::template_tx_msg(storage, env, 0, H256::default(), &[]);
        tx.transaction = tx_kind;
        let mut outcome = sync::execute_transaction_message::<AuroraModExp, _>(
            storage,
            &runner.wasm_runner,
            tx,
            None,
        )
        .unwrap();
        let key = storage::bytes_to_key(storage::KeyPrefix::Nep141Erc20Map, b"wrap.near");
        outcome.diff.modify(key, wnear_address.as_bytes().to_vec());
        let key =
            storage::bytes_to_key(storage::KeyPrefix::Erc20Nep141Map, wnear_address.as_bytes());
        outcome.diff.modify(key, b"wrap.near".to_vec());
        standalone::storage::commit(storage, &outcome);
    }

    // Setup xcc precompile in standalone runner
    let xcc_router_bytes = crate::tests::xcc::contract_bytes();
    let factory_update = {
        runner.env.block_height += 1;
        runner.env.predecessor_account_id = "aurora".parse().unwrap();
        runner.env.signer_account_id = "aurora".parse().unwrap();
        let storage = &mut runner.storage;
        let env = &runner.env;

        let tx_kind = sync::types::TransactionKind::new_factory_update(xcc_router_bytes);
        let mut tx =
            standalone::StandaloneRunner::template_tx_msg(storage, env, 0, H256::default(), &[]);
        tx.transaction = tx_kind;
        tx
    };
    let outcome = sync::execute_transaction_message::<AuroraModExp, _>(
        &runner.storage,
        &runner.wasm_runner,
        factory_update,
        None,
    )
    .unwrap();
    standalone::storage::commit(&mut runner.storage, &outcome);
    let set_wnear_address = {
        runner.env.block_height += 1;
        let storage = &mut runner.storage;
        let env = &runner.env;

        let tx_kind = sync::types::TransactionKind::new_factory_set_wnear_address(wnear_address);
        let mut tx =
            standalone::StandaloneRunner::template_tx_msg(storage, env, 0, H256::default(), &[]);
        tx.transaction = tx_kind;
        tx
    };
    let outcome = sync::execute_transaction_message::<AuroraModExp, _>(
        &runner.storage,
        &runner.wasm_runner,
        set_wnear_address,
        None,
    )
    .unwrap();
    standalone::storage::commit(&mut runner.storage, &outcome);

    // User calls XCC precompile
    let promise = PromiseCreateArgs {
        target_account_id: "some_account.near".parse().unwrap(),
        method: "whatever".into(),
        args: Vec::new(),
        attached_balance: Yocto::new(1),
        attached_gas: NearGas::new(100_000_000_000_000),
    };
    let xcc_args = CrossContractCallArgs::Delayed(PromiseArgs::Create(promise));
    let tx = aurora_engine_transactions::legacy::TransactionLegacy {
        nonce: signer.use_nonce().into(),
        gas_price: U256::zero(),
        gas_limit: u64::MAX.into(),
        to: Some(xcc_address),
        value: Wei::zero(),
        data: borsh::to_vec(&xcc_args).unwrap(),
    };
    let (standalone_result, call_tracer) = runner
        .submit_transaction_with_call_stack_tracing(&signer.secret_key, tx)
        .unwrap();
    let mut call_tracer = call_tracer.unwrap();
    assert!(standalone_result.status.is_ok());
    assert_eq!(call_tracer.call_stack.len(), 1);

    let trace = call_tracer.call_stack.pop().unwrap();
    assert_eq!(trace.calls.len(), 1);
    let subcall = trace.calls.first().unwrap();
    assert_eq!(subcall.call_type, call_tracer::CallType::Call);
    assert_eq!(subcall.from, xcc_address);
    assert_eq!(subcall.to.unwrap(), wnear_address);
    assert_eq!(U256::from_big_endian(&subcall.output), U256::one());

    runner.close();
}

/// A convenience function for pulling out a sub-call from a trace.
/// The `path` gives the index to pull out of each `calls` array.
/// For example `path == []` simply returns the given `root`, while
/// `path == [2, 0]` will return `root.calls[2].calls[0]`.
fn subcall_lense<'a>(
    root: &'a call_tracer::CallFrame,
    path: &[usize],
) -> &'a call_tracer::CallFrame {
    let mut result = root;
    for index in path {
        result = result.calls.get(*index).unwrap();
    }
    result
}

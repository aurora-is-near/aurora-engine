use crate::test_utils::{self, standalone};
use aurora_engine_types::{
    parameters::{CrossContractCallArgs, PromiseArgs, PromiseCreateArgs},
    storage,
    types::{Address, NearGas, Wei, Yocto},
    U256,
};
use borsh::BorshSerialize;
use engine_standalone_storage::sync;
use engine_standalone_tracing::{
    sputnik,
    types::call_tracer::{self, CallTracer},
};

#[test]
fn test_trace_precompile_direct_call() {
    let mut runner = standalone::StandaloneRunner::default();
    let mut signer = test_utils::Signer::random();

    runner.init_evm();

    let tx = aurora_engine_transactions::legacy::TransactionLegacy {
        nonce: signer.use_nonce().into(),
        gas_price: U256::zero(),
        gas_limit: u64::MAX.into(),
        to: Some(aurora_engine_precompiles::random::RandomSeed::ADDRESS),
        value: Wei::zero(),
        data: Vec::new(),
    };

    let mut listener = CallTracer::default();
    let standalone_result = sputnik::traced_call(&mut listener, || {
        runner.submit_transaction(&signer.secret_key, tx).unwrap()
    });
    assert!(standalone_result.status.is_ok());
    assert_eq!(listener.call_stack.len(), 1);

    let trace = listener.call_stack.pop().unwrap();

    let expected_trace = call_tracer::CallFrame {
        call_type: call_tracer::CallType::Call,
        from: test_utils::address_from_secret_key(&signer.secret_key),
        to: Some(aurora_engine_precompiles::random::RandomSeed::ADDRESS),
        value: U256::zero(),
        gas: u64::MAX,
        gas_used: 21000_u64,
        input: Vec::new(),
        output: [0u8; 32].to_vec(),
        error: None,
        calls: Vec::new(),
    };

    assert_eq!(trace, expected_trace);

    runner.close();
}

#[test]
fn test_trace_contract_single_call() {
    let mut runner = standalone::StandaloneRunner::default();
    let mut signer = test_utils::Signer::random();
    let signer_address = test_utils::address_from_secret_key(&signer.secret_key);

    runner.init_evm();

    let constructor = test_utils::erc20::ERC20Constructor::load();
    let deploy_tx = constructor.deploy("Test", "TST", signer.use_nonce().into());
    let deploy_result = runner
        .submit_transaction(&signer.secret_key, deploy_tx)
        .unwrap();
    let contract_address = {
        let bytes = test_utils::unwrap_success_slice(&deploy_result);
        Address::try_from_slice(bytes).unwrap()
    };
    let contract = test_utils::erc20::ERC20(constructor.0.deployed_at(contract_address));

    let tx = contract.balance_of(signer_address, signer.use_nonce().into());
    let mut listener = CallTracer::default();
    let standalone_result = sputnik::traced_call(&mut listener, || {
        runner
            .submit_transaction(&signer.secret_key, tx.clone())
            .unwrap()
    });
    assert!(standalone_result.status.is_ok());
    assert_eq!(listener.call_stack.len(), 1);

    let trace = listener.call_stack.pop().unwrap();

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
    let params = context.exact_output_single_params(OUTPUT_AMOUNT.into(), &token_a, &token_b);

    let mut listener = CallTracer::default();
    let (_amount_in, _profile) = sputnik::traced_call(&mut listener, || {
        context
            .runner
            .submit_with_signer_profiled(&mut context.signer, |nonce| {
                context.swap_router.exact_output_single(params, nonce)
            })
            .unwrap()
    });

    assert_eq!(listener.call_stack.len(), 1);

    let user_address = test_utils::address_from_secret_key(&context.signer.secret_key);
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
    let mut signer = test_utils::Signer::random();

    runner.init_evm();

    let constructor = test_utils::standard_precompiles::PrecompilesConstructor::load();
    let deploy_tx = constructor.deploy(signer.use_nonce().into());
    let deploy_result = runner
        .submit_transaction(&signer.secret_key, deploy_tx)
        .unwrap();
    let contract_address = {
        let bytes = test_utils::unwrap_success_slice(&deploy_result);
        Address::try_from_slice(bytes).unwrap()
    };
    let contract = test_utils::standard_precompiles::PrecompilesContract(
        constructor.0.deployed_at(contract_address),
    );

    // This transaction calls the standard precompiles (`ecrecover`, `sha256`, etc) one aft the other.
    // So the trace is one top-level call with multiple sub-calls (and the sub-calls contain no further sub-calls).
    let tx = contract.call_method("test_all", signer.use_nonce().into());
    let mut listener = CallTracer::default();
    let standalone_result = sputnik::traced_call(&mut listener, || {
        runner
            .submit_transaction(&signer.secret_key, tx.clone())
            .unwrap()
    });
    assert!(standalone_result.status.is_ok());
    assert_eq!(listener.call_stack.len(), 1);

    let trace = listener.call_stack.pop().unwrap();
    assert_eq!(trace.calls.len(), 8);
    for call in trace.calls {
        assert!(call.calls.is_empty());
    }

    runner.close();
}

#[test]
fn test_trace_precompiles_with_subcalls() {
    // The XCC precompile does internal sub-calls. We will trace an XCC call.

    let mut runner = standalone::StandaloneRunner::default();
    let mut signer = test_utils::Signer::random();
    let signer_address = test_utils::address_from_secret_key(&signer.secret_key);
    let xcc_address = aurora_engine_precompiles::xcc::cross_contract_call::ADDRESS;

    runner.init_evm();

    // Deploy an ERC-20 contract to act as wNEAR. It doesn't actually need to be bridged for
    // this test because we are not executing any scheduled promises.
    let constructor = test_utils::erc20::ERC20Constructor::load();
    let deploy_tx = constructor.deploy("wNEAR", "WNEAR", signer.use_nonce().into());
    let deploy_result = runner
        .submit_transaction(&signer.secret_key, deploy_tx)
        .unwrap();
    let wnear_address = {
        let bytes = test_utils::unwrap_success_slice(&deploy_result);
        Address::try_from_slice(bytes).unwrap()
    };
    let wnear = test_utils::erc20::ERC20(constructor.0.deployed_at(wnear_address));
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

        let mut tx =
            standalone::StandaloneRunner::template_tx_msg(storage, env, 0, Default::default(), &[]);
        tx.transaction = sync::types::TransactionKind::DeployErc20(
            aurora_engine::parameters::DeployErc20TokenArgs {
                nep141: "wrap.near".parse().unwrap(),
            },
        );
        let mut outcome = sync::execute_transaction_message(storage, tx).unwrap();
        let key = storage::bytes_to_key(storage::KeyPrefix::Nep141Erc20Map, b"wrap.near");
        outcome.diff.modify(key, wnear_address.as_bytes().to_vec());
        let key =
            storage::bytes_to_key(storage::KeyPrefix::Erc20Nep141Map, wnear_address.as_bytes());
        outcome.diff.modify(key, b"wrap.near".to_vec());
        test_utils::standalone::storage::commit(storage, &outcome);
    }

    // Setup xcc precompile in standalone runner
    let xcc_router_bytes = crate::tests::xcc::contract_bytes();
    let factory_update = {
        runner.env.block_height += 1;
        runner.env.predecessor_account_id = "aurora".parse().unwrap();
        runner.env.signer_account_id = "aurora".parse().unwrap();
        let storage = &mut runner.storage;
        let env = &runner.env;

        let mut tx =
            standalone::StandaloneRunner::template_tx_msg(storage, env, 0, Default::default(), &[]);
        tx.transaction = sync::types::TransactionKind::FactoryUpdate(xcc_router_bytes);
        tx
    };
    let outcome = sync::execute_transaction_message(&runner.storage, factory_update).unwrap();
    test_utils::standalone::storage::commit(&mut runner.storage, &outcome);
    let set_wnear_address = {
        runner.env.block_height += 1;
        let storage = &mut runner.storage;
        let env = &runner.env;

        let mut tx =
            standalone::StandaloneRunner::template_tx_msg(storage, env, 0, Default::default(), &[]);
        tx.transaction = sync::types::TransactionKind::FactorySetWNearAddress(wnear_address);
        tx
    };
    let outcome = sync::execute_transaction_message(&runner.storage, set_wnear_address).unwrap();
    test_utils::standalone::storage::commit(&mut runner.storage, &outcome);

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
        data: xcc_args.try_to_vec().unwrap(),
    };
    let mut listener = CallTracer::default();
    let standalone_result = sputnik::traced_call(&mut listener, || {
        runner
            .submit_transaction(&signer.secret_key, tx.clone())
            .unwrap()
    });
    assert!(standalone_result.status.is_ok());
    assert_eq!(listener.call_stack.len(), 1);

    let trace = listener.call_stack.pop().unwrap();
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
fn subcall_lense<'a, 'b>(
    root: &'a call_tracer::CallFrame,
    path: &'b [usize],
) -> &'a call_tracer::CallFrame {
    let mut result = root;
    for index in path {
        result = result.calls.get(*index).unwrap();
    }
    result
}

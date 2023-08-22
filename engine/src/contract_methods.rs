//! This module contains implementations for all top-level functions in the Aurora Engine
//! smart contract. All functions return `Result<(), ContractError>` because any output
//! is returned via the `IO` object and none of these functions are intended to panic.
//! Conditions which would cause the smart contract to panic are captured in the `ContractError`.
//! The actual panic happens via the `sdk_unwrap()` call where these functions are used in `lib.rs`.
//! The reason to isolate these implementations is so that they can be shared between both
//! the smart contract and the standalone.

use crate::{
    connector::{self, EthConnectorContract},
    engine::{self, Engine},
    errors,
    pausables::{
        Authorizer, EngineAuthorizer, EnginePrecompilesPauser, PausedPrecompilesChecker,
        PausedPrecompilesManager, PrecompileFlags,
    },
    state, xcc,
};
use aurora_engine_modexp::AuroraModExp;
use aurora_engine_sdk::{
    env::Env,
    error::ReadU64Error,
    io::{StorageIntermediate, IO},
    promise::PromiseHandler,
};
use aurora_engine_types::{
    account_id::AccountId,
    borsh::{BorshDeserialize, BorshSerialize},
    parameters::{
        connector::{
            InitCallArgs, NEP141FtOnTransferArgs, ResolveTransferCallArgs, SetContractDataCallArgs,
            StorageDepositCallArgs, StorageWithdrawCallArgs, TransferCallArgs,
            TransferCallCallArgs,
        },
        engine::{
            errors::ParseTypeFromJsonError, CallArgs, DeployErc20TokenArgs, NewCallArgs,
            PauseEthConnectorCallArgs, PausePrecompilesCallArgs, RelayerKeyArgs,
            RelayerKeyManagerArgs, SetOwnerArgs, SetUpgradeDelayBlocksArgs, SubmitArgs,
        },
        promise::{PromiseAction, PromiseBatchAction},
        RefundCallArgs,
    },
    storage::{self, KeyPrefix},
    types::{Address, PromiseResult, Yocto},
    vec, Box, Vec,
};

const CODE_KEY: &[u8; 4] = b"CODE";
const CODE_STAGE_KEY: &[u8; 10] = b"CODE_STAGE";

pub fn new<I: IO + Copy>(mut io: I) -> Result<(), ContractError> {
    if state::get_state(&io).is_ok() {
        return Err(b"ERR_ALREADY_INITIALIZED".into());
    }

    let bytes = io.read_input().to_vec();
    let args = NewCallArgs::deserialize(&bytes).map_err(|_| errors::ERR_BORSH_DESERIALIZE)?;
    state::set_state(&mut io, &args.into())?;
    Ok(())
}

pub fn get_version<I: IO>(mut io: I) -> Result<(), ContractError> {
    let version =
        option_env!("NEAR_EVM_VERSION").map_or(&include_bytes!("../../VERSION")[..], str::as_bytes);
    io.return_output(version);
    Ok(())
}

pub fn get_owner<I: IO + Copy>(mut io: I) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    io.return_output(state.owner_id.as_bytes());
    Ok(())
}

pub fn set_owner<I: IO + Copy, E: Env>(mut io: I, env: &E) -> Result<(), ContractError> {
    let mut state = state::get_state(&io)?;

    require_running(&state)?;
    require_owner_only(&state, &env.predecessor_account_id())?;

    let args: SetOwnerArgs = io.read_input_borsh()?;
    if state.owner_id == args.new_owner {
        return Err(errors::ERR_SAME_OWNER.into());
    }

    state.owner_id = args.new_owner;
    state::set_state(&mut io, &state)?;

    Ok(())
}

pub fn get_bridge_prover<I: IO + Copy>(mut io: I) -> Result<(), ContractError> {
    let connector = EthConnectorContract::init_instance(io)?;
    io.return_output(connector.get_bridge_prover().as_bytes());
    Ok(())
}

pub fn get_chain_id<I: IO + Copy>(mut io: I) -> Result<(), ContractError> {
    io.return_output(&state::get_state(&io)?.chain_id);
    Ok(())
}

pub fn get_upgrade_delay_blocks<I: IO + Copy>(mut io: I) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    io.return_output(&state.upgrade_delay_blocks.to_le_bytes());
    Ok(())
}

pub fn set_upgrade_delay_blocks<I: IO + Copy, E: Env>(
    mut io: I,
    env: &E,
) -> Result<(), ContractError> {
    let mut state = state::get_state(&io)?;
    require_running(&state)?;
    require_owner_only(&state, &env.predecessor_account_id())?;
    let args: SetUpgradeDelayBlocksArgs = io.read_input_borsh()?;
    state.upgrade_delay_blocks = args.upgrade_delay_blocks;
    state::set_state(&mut io, &state)?;
    Ok(())
}

pub fn get_upgrade_index<I: IO + Copy>(mut io: I) -> Result<(), ContractError> {
    let index = internal_get_upgrade_index(&io)?;
    io.return_output(&index.to_le_bytes());
    Ok(())
}

pub fn stage_upgrade<I: IO + Copy, E: Env>(mut io: I, env: &E) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    let delay_block_height = env.block_height() + state.upgrade_delay_blocks;
    require_owner_only(&state, &env.predecessor_account_id())?;
    io.read_input_and_store(&storage::bytes_to_key(KeyPrefix::Config, CODE_KEY));
    io.write_storage(
        &storage::bytes_to_key(KeyPrefix::Config, CODE_STAGE_KEY),
        &delay_block_height.to_le_bytes(),
    );
    Ok(())
}

pub fn resume_precompiles<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    let predecessor_account_id = env.predecessor_account_id();

    require_owner_only(&state, &predecessor_account_id)?;

    let args: PausePrecompilesCallArgs = io.read_input_borsh()?;
    let flags = PrecompileFlags::from_bits_truncate(args.paused_mask);
    let mut pauser = EnginePrecompilesPauser::from_io(io);
    pauser.resume_precompiles(flags);
    Ok(())
}

pub fn pause_precompiles<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    let authorizer: EngineAuthorizer = engine::get_authorizer(&io);

    if !authorizer.is_authorized(&env.predecessor_account_id()) {
        return Err(b"ERR_UNAUTHORIZED".into());
    }

    let args: PausePrecompilesCallArgs = io.read_input_borsh()?;
    let flags = PrecompileFlags::from_bits_truncate(args.paused_mask);
    let mut pauser = EnginePrecompilesPauser::from_io(io);
    pauser.pause_precompiles(flags);
    Ok(())
}

pub fn paused_precompiles<I: IO + Copy>(mut io: I) -> Result<(), ContractError> {
    let pauser = EnginePrecompilesPauser::from_io(io);
    let data = pauser.paused().bits().to_le_bytes();
    io.return_output(&data[..]);
    Ok(())
}

pub fn pause_contract<I: IO + Copy, E: Env>(mut io: I, env: &E) -> Result<(), ContractError> {
    let mut state = state::get_state(&io)?;
    require_owner_only(&state, &env.predecessor_account_id())?;
    if state.is_paused {
        return Err(errors::ERR_PAUSED.into());
    }
    state.is_paused = true;
    state::set_state(&mut io, &state)?;
    Ok(())
}

pub fn resume_contract<I: IO + Copy, E: Env>(mut io: I, env: &E) -> Result<(), ContractError> {
    let mut state = state::get_state(&io)?;
    require_owner_only(&state, &env.predecessor_account_id())?;
    if !state.is_paused {
        return Err(errors::ERR_RUNNING.into());
    }
    state.is_paused = false;
    state::set_state(&mut io, &state)?;
    Ok(())
}

pub fn deploy_code<'env, I: IO + Copy, E: Env, H: PromiseHandler>(
    mut io: I,
    env: &'env E,
    handler: &mut H,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    let input = io.read_input().to_vec();
    let current_account_id = env.current_account_id();
    let mut engine: Engine<'env, I, E, AuroraModExp> = Engine::new_with_state(
        state,
        predecessor_address(&env.predecessor_account_id()),
        current_account_id,
        io,
        env,
    );
    let result = engine.deploy_code_with_input(input, handler)?;
    let result_bytes = result.try_to_vec().map_err(|_| errors::ERR_SERIALIZE)?;
    io.return_output(&result_bytes);
    Ok(())
}

pub fn call<'env, I: IO + Copy, E: Env, H: PromiseHandler>(
    mut io: I,
    env: &'env E,
    handler: &mut H,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    let bytes = io.read_input().to_vec();
    let args = CallArgs::deserialize(&bytes).ok_or(errors::ERR_BORSH_DESERIALIZE)?;
    let current_account_id = env.current_account_id();
    let predecessor_account_id = env.predecessor_account_id();

    // During the XCC flow the Engine will call itself to move wNEAR
    // to the user's sub-account. We do not want this move to happen
    // if prior promises in the flow have failed.
    if current_account_id == predecessor_account_id {
        let check_promise: Result<(), &[u8]> = match handler.promise_result_check() {
            Some(true) | None => Ok(()),
            Some(false) => Err(b"ERR_CALLBACK_OF_FAILED_PROMISE"),
        };
        check_promise?;
    }

    let mut engine: Engine<'env, I, E, AuroraModExp> = Engine::new_with_state(
        state,
        predecessor_address(&predecessor_account_id),
        current_account_id,
        io,
        env,
    );
    let result = engine.call_with_args(args, handler)?;
    let result_bytes = result.try_to_vec().map_err(|_| errors::ERR_SERIALIZE)?;
    io.return_output(&result_bytes);
    Ok(())
}

pub fn submit<I: IO + Copy, E: Env, H: PromiseHandler>(
    mut io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    let tx_data = io.read_input().to_vec();
    let current_account_id = env.current_account_id();
    let relayer_address = predecessor_address(&env.predecessor_account_id());
    let args = SubmitArgs {
        tx_data,
        ..Default::default()
    };
    let result = engine::submit(
        io,
        env,
        &args,
        state,
        current_account_id,
        relayer_address,
        handler,
    )?;
    let result_bytes = result.try_to_vec().map_err(|_| errors::ERR_SERIALIZE)?;
    io.return_output(&result_bytes);

    Ok(())
}

pub fn submit_with_args<I: IO + Copy, E: Env, H: PromiseHandler>(
    mut io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    let args: SubmitArgs = io.read_input_borsh()?;
    let current_account_id = env.current_account_id();
    let relayer_address = predecessor_address(&env.predecessor_account_id());
    let result = engine::submit(
        io,
        env,
        &args,
        state,
        current_account_id,
        relayer_address,
        handler,
    )?;
    let result_bytes = result.try_to_vec().map_err(|_| errors::ERR_SERIALIZE)?;
    io.return_output(&result_bytes);

    Ok(())
}

pub fn register_relayer<'env, I: IO + Copy, E: Env>(
    io: I,
    env: &'env E,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    let relayer_address = io.read_input_arr20()?;

    let current_account_id = env.current_account_id();
    let predecessor_account_id = env.predecessor_account_id();
    let mut engine: Engine<'env, I, E, AuroraModExp> = Engine::new_with_state(
        state,
        predecessor_address(&predecessor_account_id),
        current_account_id,
        io,
        env,
    );
    engine.register_relayer(
        predecessor_account_id.as_bytes(),
        Address::from_array(relayer_address),
    );
    Ok(())
}

pub fn factory_update<I: IO + Copy, E: Env>(mut io: I, env: &E) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    require_owner_only(&state, &env.predecessor_account_id())?;
    let bytes = io.read_input().to_vec();
    let router_bytecode = xcc::RouterCode::new(bytes);
    xcc::update_router_code(&mut io, &router_bytecode);
    Ok(())
}

pub fn factory_update_address_version<I: IO + Copy, E: Env, H: PromiseHandler>(
    mut io: I,
    env: &E,
    handler: &H,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    // The function is only set to be private, otherwise callback error will happen.
    env.assert_private_call()?;
    let check_deploy: Result<(), &[u8]> = match handler.promise_result_check() {
        Some(true) => Ok(()),
        Some(false) => Err(b"ERR_ROUTER_DEPLOY_FAILED"),
        None => Err(b"ERR_ROUTER_UPDATE_NOT_CALLBACK"),
    };
    check_deploy?;
    let args: xcc::AddressVersionUpdateArgs = io.read_input_borsh()?;
    xcc::set_code_version_of_address(&mut io, &args.address, args.version);
    Ok(())
}

pub fn factory_set_wnear_address<I: IO + Copy, E: Env>(
    mut io: I,
    env: &E,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    require_owner_only(&state, &env.predecessor_account_id())?;
    let address = io.read_input_arr20()?;
    xcc::set_wnear_address(&mut io, &Address::from_array(address));
    Ok(())
}

pub fn factory_get_wnear_address<I: IO + Copy>(mut io: I) -> Result<(), ContractError> {
    let address = aurora_engine_precompiles::xcc::state::get_wnear_address(&io);
    let bytes = address.try_to_vec().map_err(|_| errors::ERR_SERIALIZE)?;
    io.return_output(&bytes);
    Ok(())
}

pub fn fund_xcc_sub_account<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: &I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    let state = state::get_state(io)?;
    require_running(&state)?;
    // This method can only be called by the owner because it allows specifying the
    // account ID of the wNEAR account. This information must be accurate for the
    // sub-account to work properly, therefore this method can only be called by
    // a trusted user.
    require_owner_only(&state, &env.predecessor_account_id())?;
    let args: xcc::FundXccArgs = io.read_input_borsh()?;
    xcc::fund_xcc_sub_account(io, handler, env, args)?;
    Ok(())
}

pub fn ft_on_transfer<'env, I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &'env E,
    handler: &mut H,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    let current_account_id = env.current_account_id();
    let predecessor_account_id = env.predecessor_account_id();
    let mut engine: Engine<'env, I, E, AuroraModExp> = Engine::new_with_state(
        state,
        predecessor_address(&predecessor_account_id),
        current_account_id.clone(),
        io,
        env,
    );

    let args: NEP141FtOnTransferArgs = serde_json::from_slice(&io.read_input().to_vec())
        .map_err(Into::<ParseTypeFromJsonError>::into)?;

    if predecessor_account_id == current_account_id {
        EthConnectorContract::init_instance(io)?.ft_on_transfer(&engine, &args)?;
    } else {
        engine.receive_erc20_tokens(&predecessor_account_id, &args, &current_account_id, handler);
    }
    Ok(())
}

pub fn deploy_erc20_token<I: IO + Copy, E: Env, H: PromiseHandler>(
    mut io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    // Id of the NEP141 token in Near
    let args: DeployErc20TokenArgs = io.read_input_borsh()?;

    let address = engine::deploy_erc20_token(args, io, env, handler)?;

    io.return_output(
        &address
            .as_bytes()
            .try_to_vec()
            .map_err(|_| errors::ERR_SERIALIZE)?,
    );
    Ok(())
}

pub fn refund_on_error<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    env.assert_private_call()?;

    // This function should only be called as the callback of
    // exactly one promise.
    if handler.promise_results_count() != 1 {
        return Err(errors::ERR_PROMISE_COUNT.into());
    }

    if let Some(PromiseResult::Successful(_)) = handler.promise_result(0) {
        // Promise succeeded -- nothing to do
    } else {
        // Exit call failed; need to refund tokens
        let args: RefundCallArgs = io.read_input_borsh()?;
        let refund_result = engine::refund_on_error(io, env, state, &args, handler)?;

        if !refund_result.status.is_ok() {
            return Err(errors::ERR_REFUND_FAILURE.into());
        }
    }
    Ok(())
}

pub fn set_key_manager<I: IO + Copy, E: Env>(mut io: I, env: &E) -> Result<(), ContractError> {
    let mut state = state::get_state(&io)?;

    require_running(&state)?;
    require_owner_only(&state, &env.predecessor_account_id())?;

    let key_manager = serde_json::from_slice::<RelayerKeyManagerArgs>(&io.read_input().to_vec())
        .map(|args| args.key_manager)
        .map_err(|_| errors::ERR_JSON_DESERIALIZE)?;

    if state.key_manager == key_manager {
        return Err(errors::ERR_SAME_KEY_MANAGER.into());
    }

    state.key_manager = key_manager;
    state::set_state(&mut io, &state)?;

    Ok(())
}

pub fn add_relayer_key<I: IO + Copy, E: Env, H: PromiseHandler>(
    mut io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;

    require_running(&state)?;
    require_key_manager_only(&state, &env.predecessor_account_id())?;

    let public_key = serde_json::from_slice::<RelayerKeyArgs>(&io.read_input().to_vec())
        .map(|args| args.public_key)
        .map_err(|_| errors::ERR_JSON_DESERIALIZE)?;
    let allowance = Yocto::new(env.attached_deposit());
    aurora_engine_sdk::log!("attached key allowance: {allowance}");

    if allowance.as_u128() < 100 {
        // TODO: Clarify the minimum amount if check is needed then change error type
        return Err(errors::ERR_NOT_ALLOWED.into());
    }

    engine::add_function_call_key(&mut io, &public_key);

    let current_account_id = env.current_account_id();
    let action = PromiseAction::AddFunctionCallKey {
        public_key,
        allowance,
        nonce: 0, // not actually used - depends on block height
        receiver_id: current_account_id.clone(),
        function_names: "call,submit,submit_with_args".into(),
    };
    let promise = PromiseBatchAction {
        target_account_id: current_account_id,
        actions: vec![action],
    };

    let promise_id = unsafe { handler.promise_create_batch(&promise) };
    handler.promise_return(promise_id);

    Ok(())
}

pub fn remove_relayer_key<I: IO + Copy, E: Env, H: PromiseHandler>(
    mut io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;

    require_running(&state)?;
    require_key_manager_only(&state, &env.predecessor_account_id())?;

    let args: RelayerKeyArgs = serde_json::from_slice(&io.read_input().to_vec())
        .map_err(|_| errors::ERR_JSON_DESERIALIZE)?;

    engine::remove_function_call_key(&mut io, &args.public_key)?;

    let action = PromiseAction::DeleteKey {
        public_key: args.public_key,
    };
    let promise = PromiseBatchAction {
        target_account_id: env.current_account_id(),
        actions: vec![action],
    };

    let promise_id = unsafe { handler.promise_create_batch(&promise) };
    handler.promise_return(promise_id);

    Ok(())
}

pub fn new_eth_connector<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    // Only the owner can initialize the EthConnector
    let is_private = env.assert_private_call();
    if is_private.is_err() {
        require_owner_only(&state, &env.predecessor_account_id())?;
    }

    let args: InitCallArgs = io.read_input_borsh()?;
    let owner_id = env.current_account_id();

    EthConnectorContract::create_contract(io, &owner_id, args)?;
    Ok(())
}

pub fn set_eth_connector_contract_data<I: IO + Copy, E: Env>(
    mut io: I,
    env: &E,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    // Only the owner can set the EthConnector contract data
    let is_private = env.assert_private_call();
    if is_private.is_err() {
        require_owner_only(&state, &env.predecessor_account_id())?;
    }

    let args: SetContractDataCallArgs = io.read_input_borsh()?;
    connector::set_contract_data(&mut io, args)?;
    Ok(())
}

pub fn withdraw<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<Vec<u8>, ContractError> {
    require_running(&state::get_state(&io)?)?;
    env.assert_one_yocto()?;
    let args = io.read_input_borsh()?;
    let current_account_id = env.current_account_id();
    let predecessor_account_id = env.predecessor_account_id();
    let result = EthConnectorContract::init_instance(io)?.withdraw_eth_from_near(
        &current_account_id,
        &predecessor_account_id,
        &args,
    )?;
    let result_bytes = result.try_to_vec().map_err(|_| errors::ERR_SERIALIZE)?;

    // We only return the output via IO in the case of standalone.
    // In the case of contract we intentionally avoid IO to call Wasm directly.
    #[cfg(not(feature = "contract"))]
    {
        let mut io = io;
        io.return_output(&result_bytes);
    }

    Ok(result_bytes)
}

pub fn deposit<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    let raw_proof = io.read_input().to_vec();
    let current_account_id = env.current_account_id();
    let predecessor_account_id = env.predecessor_account_id();
    let promise_args = EthConnectorContract::init_instance(io)?.deposit(
        raw_proof,
        current_account_id,
        predecessor_account_id,
    )?;
    // Safety: this call is safe because it comes from the eth-connector, not users.
    // The call is to verify the user-supplied proof for the deposit, with `finish_deposit`
    // as a callback.
    let promise_id = unsafe { handler.promise_create_with_callback(&promise_args) };
    handler.promise_return(promise_id);
    Ok(())
}

pub fn finish_deposit<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    env.assert_private_call()?;

    // Check result from proof verification call
    if handler.promise_results_count() != 1 {
        return Err(errors::ERR_PROMISE_COUNT.into());
    }
    let promise_result = match handler.promise_result(0) {
        Some(PromiseResult::Successful(bytes)) => {
            bool::try_from_slice(&bytes).map_err(|_| errors::ERR_PROMISE_ENCODING)?
        }
        _ => return Err(errors::ERR_PROMISE_FAILED.into()),
    };
    if !promise_result {
        return Err(errors::ERR_VERIFY_PROOF.into());
    }

    let data = io.read_input_borsh()?;
    let current_account_id = env.current_account_id();
    let predecessor_account_id = env.predecessor_account_id();
    let maybe_promise_args = EthConnectorContract::init_instance(io)?.finish_deposit(
        predecessor_account_id,
        current_account_id,
        data,
        env.prepaid_gas(),
    )?;

    if let Some(promise_args) = maybe_promise_args {
        // Safety: this call is safe because it comes from the eth-connector, not users.
        // The call will be to the Engine's ft_transfer_call`, which is needed as part
        // of the bridge flow (if depositing ETH to an Aurora address).
        let promise_id = unsafe { handler.promise_create_with_callback(&promise_args) };
        handler.promise_return(promise_id);
    }

    Ok(())
}

pub fn ft_transfer<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    env.assert_one_yocto()?;
    let predecessor_account_id = env.predecessor_account_id();
    let args: TransferCallArgs = serde_json::from_slice(&io.read_input().to_vec())
        .map_err(Into::<ParseTypeFromJsonError>::into)?;
    EthConnectorContract::init_instance(io)?.ft_transfer(&predecessor_account_id, &args)?;
    Ok(())
}

pub fn ft_resolve_transfer<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &H,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;

    env.assert_private_call()?;
    if handler.promise_results_count() != 1 {
        return Err(errors::ERR_PROMISE_COUNT.into());
    }

    let args: ResolveTransferCallArgs = io.read_input().to_value()?;
    let promise_result = handler
        .promise_result(0)
        .ok_or(errors::ERR_PROMISE_ENCODING)?;

    EthConnectorContract::init_instance(io)?.ft_resolve_transfer(&args, promise_result);
    Ok(())
}

pub fn ft_transfer_call<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    // Check is payable
    env.assert_one_yocto()?;

    let args: TransferCallCallArgs = serde_json::from_slice(&io.read_input().to_vec())
        .map_err(Into::<ParseTypeFromJsonError>::into)?;
    let current_account_id = env.current_account_id();
    let predecessor_account_id = env.predecessor_account_id();
    let promise_args = EthConnectorContract::init_instance(io)?.ft_transfer_call(
        predecessor_account_id,
        current_account_id,
        args,
        env.prepaid_gas(),
    )?;
    // Safety: this call is safe. It is required by the NEP-141 spec that `ft_transfer_call`
    // creates a call to another contract's `ft_on_transfer` method.
    let promise_id = unsafe { handler.promise_create_with_callback(&promise_args) };
    handler.promise_return(promise_id);
    Ok(())
}

pub fn storage_deposit<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    let args: StorageDepositCallArgs = serde_json::from_slice(&io.read_input().to_vec())
        .map_err(Into::<ParseTypeFromJsonError>::into)?;
    let predecessor_account_id = env.predecessor_account_id();
    let amount = Yocto::new(env.attached_deposit());
    let maybe_promise = EthConnectorContract::init_instance(io)?.storage_deposit(
        predecessor_account_id,
        amount,
        args,
    )?;
    if let Some(promise) = maybe_promise {
        // Safety: This call is safe. It is only a transfer back to the user in the case
        // that they over paid for their deposit.
        unsafe { handler.promise_create_batch(&promise) };
    }
    Ok(())
}

pub fn storage_unregister<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    env.assert_one_yocto()?;
    let predecessor_account_id = env.predecessor_account_id();
    let force = serde_json::from_slice::<serde_json::Value>(&io.read_input().to_vec())
        .ok()
        .and_then(|args| args["force"].as_bool());
    let maybe_promise = EthConnectorContract::init_instance(io)?
        .storage_unregister(predecessor_account_id, force)?;
    if let Some(promise) = maybe_promise {
        // Safety: This call is safe. It is only a transfer back to the user for their deposit.
        unsafe { handler.promise_create_batch(&promise) };
    }
    Ok(())
}

pub fn storage_withdraw<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    env.assert_one_yocto()?;
    let args: StorageWithdrawCallArgs = serde_json::from_slice(&io.read_input().to_vec())
        .map_err(Into::<ParseTypeFromJsonError>::into)?;
    let predecessor_account_id = env.predecessor_account_id();
    EthConnectorContract::init_instance(io)?.storage_withdraw(&predecessor_account_id, &args)?;
    Ok(())
}

pub fn set_paused_flags<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    let is_private = env.assert_private_call();
    if is_private.is_err() {
        require_owner_only(&state, &env.predecessor_account_id())?;
    }
    let args: PauseEthConnectorCallArgs = io.read_input_borsh()?;
    EthConnectorContract::init_instance(io)?.set_paused_flags(&args);
    Ok(())
}

fn internal_get_upgrade_index<I: IO>(io: &I) -> Result<u64, ContractError> {
    match io.read_u64(&storage::bytes_to_key(KeyPrefix::Config, CODE_STAGE_KEY)) {
        Ok(index) => Ok(index),
        Err(ReadU64Error::InvalidU64) => Err(errors::ERR_INVALID_UPGRADE.into()),
        Err(ReadU64Error::MissingValue) => Err(errors::ERR_NO_UPGRADE.into()),
    }
}

fn require_running(state: &crate::state::EngineState) -> Result<(), ContractError> {
    if state.is_paused {
        return Err(errors::ERR_PAUSED.into());
    }
    Ok(())
}

fn require_owner_only(
    state: &crate::state::EngineState,
    predecessor_account_id: &AccountId,
) -> Result<(), ContractError> {
    if &state.owner_id != predecessor_account_id {
        return Err(errors::ERR_NOT_ALLOWED.into());
    }
    Ok(())
}

fn require_key_manager_only(
    state: &state::EngineState,
    predecessor_account_id: &AccountId,
) -> Result<(), ContractError> {
    let key_manager = state
        .key_manager
        .as_ref()
        .ok_or(errors::ERR_KEY_MANAGER_IS_NOT_SET)?;
    if key_manager != predecessor_account_id {
        return Err(errors::ERR_NOT_ALLOWED.into());
    }
    Ok(())
}

fn predecessor_address(predecessor_account_id: &AccountId) -> Address {
    aurora_engine_sdk::types::near_account_to_evm_address(predecessor_account_id.as_bytes())
}

pub struct ContractError {
    pub message: Box<dyn AsRef<[u8]>>,
}

impl ContractError {
    #[must_use]
    pub fn msg(self) -> ErrorMessage {
        ErrorMessage {
            message: self.message,
        }
    }
}

impl<T: AsRef<[u8]> + 'static> From<T> for ContractError {
    fn from(value: T) -> Self {
        Self {
            message: Box::new(value),
        }
    }
}

/// This type is structurally the same as `ContractError`, but
/// importantly `ContractError` implements `From<T: AsRef<[u8]>>`
/// for easy usage in the this module's function implementations, while
/// `ErrorMessage` implements `AsRef<[u8]>` for compatibility with
/// `sdk_unwrap`.
pub struct ErrorMessage {
    pub message: Box<dyn AsRef<[u8]>>,
}

impl AsRef<[u8]> for ErrorMessage {
    fn as_ref(&self) -> &[u8] {
        self.message.as_ref().as_ref()
    }
}

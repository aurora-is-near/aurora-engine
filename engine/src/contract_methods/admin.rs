//! This module contains implementations for all top-level functions in the Aurora Engine
//! smart contract. All functions return `Result<(), ContractError>` because any output
//! is returned via the `IO` object and none of these functions are intended to panic.
//! Conditions which would cause the smart contract to panic are captured in the `ContractError`.
//! The actual panic happens via the `sdk_unwrap()` call where these functions are used in `lib.rs`.
//! The reason to isolate these implementations is so that they can be shared between both
//! the smart contract and the standalone.

use crate::{
    connector::EthConnectorContract,
    contract_methods::{
        predecessor_address, require_key_manager_only, require_owner_only, require_running,
        ContractError,
    },
    engine::{self, Engine},
    errors,
    pausables::{
        Authorizer, EngineAuthorizer, EnginePrecompilesPauser, PausedPrecompilesChecker,
        PausedPrecompilesManager, PrecompileFlags,
    },
    state,
};
use aurora_engine_modexp::AuroraModExp;
use aurora_engine_sdk::{
    env::Env,
    error::ReadU64Error,
    io::{StorageIntermediate, IO},
    promise::PromiseHandler,
};
use aurora_engine_types::{
    parameters::{
        engine::{
            NewCallArgs, PausePrecompilesCallArgs, RelayerKeyArgs, RelayerKeyManagerArgs,
            SetOwnerArgs, SetUpgradeDelayBlocksArgs,
        },
        promise::{PromiseAction, PromiseBatchAction},
    },
    storage::{self, KeyPrefix},
    types::{Address, Yocto},
    vec,
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
    let version = option_env!("NEAR_EVM_VERSION")
        .map_or(&include_bytes!("../../../VERSION")[..], str::as_bytes);
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

fn internal_get_upgrade_index<I: IO>(io: &I) -> Result<u64, ContractError> {
    match io.read_u64(&storage::bytes_to_key(KeyPrefix::Config, CODE_STAGE_KEY)) {
        Ok(index) => Ok(index),
        Err(ReadU64Error::InvalidU64) => Err(errors::ERR_INVALID_UPGRADE.into()),
        Err(ReadU64Error::MissingValue) => Err(errors::ERR_NO_UPGRADE.into()),
    }
}

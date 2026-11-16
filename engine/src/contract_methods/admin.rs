//! This module contains implementations for all top-level functions in the Aurora Engine
//! smart contract. All functions return `Result<(), ContractError>` because any output
//! is returned via the `IO` object and none of these functions are intended to panic.
//! Conditions which would cause the smart contract to panic are captured in the `ContractError`.
//! The actual panic happens via the `sdk_unwrap()` call where these functions are used in `lib.rs`.
//! The reason to isolate these implementations is so that they can be shared between both
//! the smart contract and the standalone.

use aurora_engine_hashchain::{bloom::Bloom, hashchain::Hashchain};
use aurora_engine_modexp::AuroraModExp;
use aurora_engine_sdk::{
    env::Env,
    error::ReadU64Error,
    io::{IO, StorageIntermediate},
    promise::PromiseHandler,
};
use aurora_engine_types::parameters::admin::AdminCallArgs;
use aurora_engine_types::parameters::engine::{FullAccessKeyArgs, UpgradeParams};
use aurora_engine_types::types::{NearGas, ZERO_YOCTO};
use aurora_engine_types::{
    ToString,
    borsh::BorshDeserialize,
    parameters::{
        engine::{
            NewCallArgs, PausePrecompilesCallArgs, RelayerKeyArgs, RelayerKeyManagerArgs,
            SetOwnerArgs, SetUpgradeDelayBlocksArgs, StartHashchainArgs,
        },
        promise::{PromiseAction, PromiseBatchAction},
    },
    storage::{self, KeyPrefix},
    types::{Address, Yocto},
    vec,
};
use function_name::named;

use crate::{
    contract_methods::{
        ContractError, predecessor_address, require_key_manager_only, require_owner_only,
        require_paused, require_running,
    },
    engine::{self, Engine},
    errors,
    hashchain::with_hashchain,
    pausables::{
        Authorizer, EngineAuthorizer, EnginePrecompilesPauser, PausedPrecompilesChecker,
        PausedPrecompilesManager, PrecompileFlags,
    },
    state::{self, EngineState},
};

const CODE_KEY: &[u8; 4] = b"CODE";
const CODE_STAGE_KEY: &[u8; 10] = b"CODE_STAGE";
const GAS_FOR_STATE_MIGRATION: NearGas = NearGas::new(50_000_000_000_000);
const GAS_FOR_STORING_RELAYER_KEY: NearGas = NearGas::new(5_000_000_000_000);

#[named]
pub fn new<I: IO + Copy, E: Env>(mut io: I, env: &E) -> Result<(), ContractError> {
    if state::get_state(&io).is_ok() {
        return Err(b"ERR_ALREADY_INITIALIZED".into());
    }

    let input = io.read_input().to_vec();
    let args = NewCallArgs::deserialize(&input).map_err(|_| errors::ERR_BORSH_DESERIALIZE)?;

    let initial_hashchain = args.initial_hashchain();
    let state: EngineState = args.into();

    if let Some(block_hashchain) = initial_hashchain {
        let block_height = env.block_height();
        let mut hashchain = Hashchain::new(
            state.chain_id,
            env.current_account_id(),
            block_height,
            block_hashchain,
        );

        hashchain.add_block_tx(
            block_height,
            function_name!(),
            &input,
            &[],
            &Bloom::default(),
        )?;
        crate::hashchain::save_hashchain(&mut io, &hashchain)?;
    }

    state::set_state(&mut io, &state)?;
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

#[named]
pub fn set_owner<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
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
    })
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

#[named]
pub fn set_upgrade_delay_blocks<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
        let mut state = state::get_state(&io)?;
        require_running(&state)?;
        require_owner_only(&state, &env.predecessor_account_id())?;
        let args: SetUpgradeDelayBlocksArgs = io.read_input_borsh()?;
        state.upgrade_delay_blocks = args.upgrade_delay_blocks;
        state::set_state(&mut io, &state)?;
        Ok(())
    })
}

pub fn get_upgrade_index<I: IO + Copy>(mut io: I) -> Result<(), ContractError> {
    let index = internal_get_upgrade_index(&io)?;
    io.return_output(&index.to_le_bytes());
    Ok(())
}

#[named]
pub fn stage_upgrade<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
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
    })
}

pub fn upgrade<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    require_owner_only(&state, &env.predecessor_account_id())?;

    let input = io.read_input().to_vec();
    let (code, state_migration_gas) = match UpgradeParams::try_from_slice(&input) {
        Ok(args) => (
            args.code,
            args.state_migration_gas
                .map_or(GAS_FOR_STATE_MIGRATION, NearGas::new),
        ),
        Err(_) => (input, GAS_FOR_STATE_MIGRATION), // Backward compatibility
    };

    let target_account_id = env.current_account_id();
    let batch = PromiseBatchAction {
        target_account_id,
        actions: vec![
            PromiseAction::DeployContract { code },
            PromiseAction::FunctionCall {
                name: "state_migration".to_string(),
                args: vec![],
                attached_yocto: ZERO_YOCTO,
                gas: state_migration_gas,
            },
        ],
    };
    let promise_id = handler.promise_create_batch(&batch);

    handler.promise_return(promise_id);

    Ok(())
}

#[named]
pub fn resume_precompiles<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        let predecessor_account_id = env.predecessor_account_id();

        require_owner_only(&state, &predecessor_account_id)?;

        let args: PausePrecompilesCallArgs = io.read_input_borsh()?;
        let flags = PrecompileFlags::from_bits_truncate(args.paused_mask);
        let mut pauser = EnginePrecompilesPauser::from_io(io);
        pauser.resume_precompiles(flags);
        Ok(())
    })
}

#[named]
pub fn pause_precompiles<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
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
    })
}

pub fn paused_precompiles<I: IO + Copy>(mut io: I) -> Result<(), ContractError> {
    let pauser = EnginePrecompilesPauser::from_io(io);
    let data = pauser.paused().bits().to_le_bytes();
    io.return_output(&data[..]);
    Ok(())
}

#[named]
pub fn pause_contract<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
        let mut state = state::get_state(&io)?;
        require_owner_only(&state, &env.predecessor_account_id())?;
        require_running(&state)?;
        state.is_paused = true;
        state::set_state(&mut io, &state)?;
        Ok(())
    })
}

#[named]
pub fn resume_contract<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
        let mut state = state::get_state(&io)?;
        require_owner_only(&state, &env.predecessor_account_id())?;
        require_paused(&state)?;
        state.is_paused = false;
        state::set_state(&mut io, &state)?;
        Ok(())
    })
}

#[named]
pub fn set_key_manager<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
        let mut state = state::get_state(&io)?;

        require_running(&state)?;
        require_owner_only(&state, &env.predecessor_account_id())?;

        let key_manager =
            serde_json::from_slice::<RelayerKeyManagerArgs>(&io.read_input().to_vec())
                .map(|args| args.key_manager)
                .map_err(|_| errors::ERR_JSON_DESERIALIZE)?;

        if state.key_manager == key_manager {
            return Err(errors::ERR_SAME_KEY_MANAGER.into());
        }

        state.key_manager = key_manager;
        state::set_state(&mut io, &state)?;

        Ok(())
    })
}

pub fn add_relayer_key<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;

    require_running(&state)?;
    require_key_manager_only(&state, &env.predecessor_account_id())?;

    let input = io.read_input().to_vec();
    let public_key = serde_json::from_slice::<RelayerKeyArgs>(&input)
        .map(|args| args.public_key)
        .map_err(|_| errors::ERR_JSON_DESERIALIZE)?;
    let allowance = Yocto::new(env.attached_deposit());
    aurora_engine_sdk::log!("attached key allowance: {allowance}");

    if allowance.as_u128() < 100 {
        // TODO: Clarify the minimum amount if check is needed then change error type
        return Err(errors::ERR_NOT_ALLOWED.into());
    }

    let current_account_id = env.current_account_id();
    let promise = PromiseBatchAction {
        target_account_id: current_account_id.clone(),
        actions: vec![
            PromiseAction::AddFunctionCallKey {
                public_key,
                allowance,
                nonce: 0, // not actually used - depends on block height
                receiver_id: current_account_id,
                function_names: "call,submit,submit_with_args".into(),
            },
            PromiseAction::FunctionCall {
                name: "store_relayer_key_callback".to_string(),
                args: input,
                attached_yocto: ZERO_YOCTO,
                gas: GAS_FOR_STORING_RELAYER_KEY,
            },
        ],
    };

    let promise_id = handler.promise_create_batch(&promise);
    handler.promise_return(promise_id);

    Ok(())
}

#[named]
pub fn store_relayer_key_callback<I: IO + Copy, E: Env>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
        let state = state::get_state(&io)?;

        require_running(&state)?;
        env.assert_private_call()?;

        let public_key = serde_json::from_slice::<RelayerKeyArgs>(&io.read_input().to_vec())
            .map(|args| args.public_key)
            .map_err(|_| errors::ERR_JSON_DESERIALIZE)?;

        aurora_engine_sdk::log!("add relayer public key: {public_key}");

        engine::add_function_call_key(&mut io, &public_key);

        Ok(())
    })
}

#[named]
pub fn remove_relayer_key<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
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

        let promise_id = handler.promise_create_batch(&promise);
        handler.promise_return(promise_id);

        Ok(())
    })
}

#[named]
pub fn register_relayer<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        let relayer_address = io.read_input_arr20()?;

        let current_account_id = env.current_account_id();
        let predecessor_account_id = env.predecessor_account_id();
        let mut engine: Engine<_, E, AuroraModExp> = Engine::new_with_state(
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
    })
}

#[named]
pub fn start_hashchain<I: IO + Copy, E: Env>(mut io: I, env: &E) -> Result<(), ContractError> {
    let mut state = state::get_state(&io)?;
    require_paused(&state)?;
    require_key_manager_only(&state, &env.predecessor_account_id())?;

    let input = io.read_input().to_vec();
    let args = StartHashchainArgs::try_from_slice(&input).map_err(|_| errors::ERR_SERIALIZE)?;
    let block_height = env.block_height();

    // Starting hashchain must be for an earlier block
    if block_height < args.block_height {
        return Err(errors::ERR_ARGS.into());
    }

    let mut hashchain = Hashchain::new(
        state.chain_id,
        env.current_account_id(),
        args.block_height + 1,
        args.block_hashchain,
    );

    if hashchain.get_current_block_height() < block_height {
        hashchain.move_to_block(block_height)?;
    }

    hashchain.add_block_tx(
        block_height,
        function_name!(),
        &input,
        &[],
        &Bloom::default(),
    )?;
    crate::hashchain::save_hashchain(&mut io, &hashchain)?;

    state.is_paused = false;
    state::set_state(&mut io, &state)?;

    Ok(())
}

pub fn get_latest_hashchain<I: IO>(io: &mut I) -> Result<(), ContractError> {
    let result = crate::hashchain::read_current_hashchain(io)?.map(|hc| {
        let block_height = hc.get_current_block_height() - 1;
        let hashchain = hex::encode(hc.get_previous_block_hashchain());
        serde_json::json!({
            "block_height": block_height,
            "hashchain": hashchain,
        })
    });

    let bytes = serde_json::to_vec(&serde_json::json!({ "result": result }))
        .map_err(|_| errors::ERR_SERIALIZE)?;
    io.return_output(&bytes);

    Ok(())
}

pub fn attach_full_access_key<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;

    require_running(&state)?;
    require_owner_only(&state, &env.predecessor_account_id())?;

    let public_key = serde_json::from_slice::<FullAccessKeyArgs>(&io.read_input().to_vec())
        .map(|args| args.public_key)
        .map_err(|_| errors::ERR_JSON_DESERIALIZE)?;
    let current_account_id = env.current_account_id();
    let action = PromiseAction::AddFullAccessKey {
        public_key,
        nonce: 0, // not actually used - depends on block height
    };
    let promise = PromiseBatchAction {
        target_account_id: current_account_id,
        actions: vec![action],
    };
    // SAFETY: This action is dangerous because it adds a new full access key (FAK) to the Engine account.
    // However, it is safe to do so here because of the `require_owner_only` check above; only the
    // (trusted) owner account can add a new FAK.
    let promise_id = handler.promise_create_batch(&promise);

    handler.promise_return(promise_id);

    Ok(())
}

/// Schedules a batch of NEAR transfers and function calls from the engine account.
///
/// Reads JSON-encoded [`AdminCallArgs`] from `io` and sends the actions, in order, to
/// `receiver_id`. Transfers and function-call deposits are funded from the engine's balance.
/// The engine must be initialized and running, the predecessor must be its owner, and the
/// call must attach a nonzero deposit in `yoctoNEAR`.
///
/// The receiver must not be the engine account itself. A receipt from the engine to itself
/// passes `assert_private_call`, so it would let the owner invoke the engine's private
/// callbacks with arbitrary arguments, and its EVM calls would be sent from the engine's own
/// address, which is the admin of the bridged ERC-20 contracts.
///
/// Sub-accounts of the engine are not restricted. This includes the XCC routers, which accept
/// `execute` calls from the engine only, so the owner can make a router run arbitrary
/// promises. It is left open so that funds stuck on a router can be recovered.
///
/// Returns the batch promise through `handler` so the caller receives its result.
/// `Ok(())` indicates that the batch was scheduled; execution happens asynchronously.
///
/// # Errors
///
/// Returns an error if the engine state cannot be loaded, the engine is paused, the
/// predecessor is not the owner, no deposit is attached, the input cannot be deserialized
/// as [`AdminCallArgs`], the receiver is the engine account, or the action list is empty.
pub fn admin_call<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;

    require_running(&state)?;
    require_owner_only(&state, &env.predecessor_account_id())?;

    if env.attached_deposit() == 0 {
        return Err(errors::ERR_DEPOSIT_REQUIRED.into());
    }

    let args = serde_json::from_slice::<AdminCallArgs>(&io.read_input().to_vec())
        .map_err(|_| errors::ERR_JSON_DESERIALIZE)?;

    if args.receiver_id == env.current_account_id() {
        return Err(errors::ERR_INVALID_RECEIVER.into());
    }

    if args.actions.is_empty() {
        return Err(errors::ERR_INVALID_ACTIONS_NUMBER.into());
    }

    let promise_id = handler.promise_create_batch(&args.into());

    handler.promise_return(promise_id);

    Ok(())
}

fn internal_get_upgrade_index<I: IO>(io: &I) -> Result<u64, ContractError> {
    match io.read_u64(&storage::bytes_to_key(KeyPrefix::Config, CODE_STAGE_KEY)) {
        Ok(index) => Ok(index),
        Err(ReadU64Error::InvalidU64) => Err(errors::ERR_INVALID_UPGRADE.into()),
        Err(ReadU64Error::MissingValue) => Err(errors::ERR_NO_UPGRADE.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurora_engine_sdk::env::Fixed;
    use aurora_engine_sdk::promise::PromiseId;
    use aurora_engine_test_doubles::io::{Storage, StoragePointer};
    use aurora_engine_test_doubles::promise::{PromiseArgs, PromiseTracker};
    use aurora_engine_types::account_id::AccountId;
    use std::cell::RefCell;
    use test_case::test_case;

    const ENGINE: &str = "aurora";
    const OWNER: &str = "owner.near";
    const STRANGER: &str = "stranger.near";
    const RECEIVER: &str = "token.near";
    const TRANSFER: &str = r#"{"Transfer":{"amount":"1"}}"#;

    fn account(id: &str) -> AccountId {
        id.parse().unwrap()
    }

    /// `admin_call` input that sends the given actions, written as JSON, to `receiver`.
    fn input(receiver: &str, actions: &str) -> Vec<u8> {
        format!(r#"{{"receiver_id":"{receiver}","actions":[{actions}]}}"#).into_bytes()
    }

    /// Storage of a running engine owned by `OWNER`, with the given call input.
    fn init_storage(input: Vec<u8>) -> RefCell<Storage> {
        let storage = RefCell::new(Storage {
            input,
            ..Storage::default()
        });
        let state = EngineState {
            owner_id: account(OWNER),
            ..EngineState::default()
        };
        state::set_state(&mut StoragePointer(&storage), &state).unwrap();

        storage
    }

    /// Environment of a call to the engine sent and signed by `predecessor`.
    fn call_env(predecessor: &str, deposit: u128) -> Fixed {
        Fixed {
            signer_account_id: account(predecessor),
            current_account_id: account(ENGINE),
            predecessor_account_id: account(predecessor),
            attached_deposit: deposit,
            ..Fixed::default()
        }
    }

    /// Runs a call that must succeed and returns the batch it scheduled. The batch must be
    /// the only promise and the one returned from the call, and storage must stay untouched.
    fn scheduled_batch(storage: &RefCell<Storage>, env: &Fixed) -> PromiseBatchAction {
        let kv_store = storage.borrow().kv_store.clone();
        let mut handler = PromiseTracker::default();

        admin_call(StoragePointer(storage), env, &mut handler).unwrap();

        assert_eq!(storage.borrow().kv_store, kv_store);
        assert_eq!(handler.scheduled_promises.len(), 1);

        let id = handler.returned_promise.map(PromiseId::raw).unwrap();
        match handler.scheduled_promises.remove(&id) {
            Some(PromiseArgs::Batch(batch)) => batch,
            other => panic!("expected the returned promise to be a batch, got {other:?}"),
        }
    }

    /// Runs a call that must fail with `expected` without scheduling a promise.
    fn assert_rejected(storage: &RefCell<Storage>, env: &Fixed, expected: &[u8]) {
        let mut handler = PromiseTracker::default();

        let err = admin_call(StoragePointer(storage), env, &mut handler).unwrap_err();

        assert_eq!(
            str::from_utf8(err.message.as_ref().as_ref()),
            str::from_utf8(expected)
        );
        assert!(handler.scheduled_promises.is_empty());
    }

    #[test]
    fn test_admin_call_schedules_batch() {
        let call = r#"{"FunctionCall":{"method_name":"f","args":"e30=","deposit":"2","gas":3}}"#;
        let storage = init_storage(input(RECEIVER, &format!("{call},{TRANSFER}")));

        // Any non-zero deposit will do, not only one yocto.
        let batch = scheduled_batch(&storage, &call_env(OWNER, 5));

        let expected = PromiseBatchAction {
            target_account_id: account(RECEIVER),
            actions: vec![
                PromiseAction::FunctionCall {
                    name: "f".to_string(),
                    args: b"{}".to_vec(),
                    attached_yocto: Yocto::new(2),
                    gas: NearGas::new(3),
                },
                PromiseAction::Transfer {
                    amount: Yocto::new(1),
                },
            ],
        };
        assert_eq!(batch, expected);
    }

    #[test]
    fn test_admin_call_accepts_engine_sub_account_as_receiver() {
        // Only the engine account itself is off limits. Its sub-accounts, such as this XCC
        // router, stay reachable so that funds stuck on them can be recovered.
        let router = "f39fd6e51aad88f6f4ce6ab8827279cfffb92266.aurora";
        let storage = init_storage(input(router, TRANSFER));

        let batch = scheduled_batch(&storage, &call_env(OWNER, 1));

        assert_eq!(batch.target_account_id, account(router));
    }

    #[test_case(STRANGER, 1, input(RECEIVER, TRANSFER), errors::ERR_NOT_ALLOWED; "caller is not the owner")]
    #[test_case(ENGINE, 1, input(RECEIVER, TRANSFER), errors::ERR_NOT_ALLOWED; "engine is not the owner")]
    #[test_case(OWNER, 0, input(RECEIVER, TRANSFER), errors::ERR_DEPOSIT_REQUIRED; "no deposit")]
    #[test_case(OWNER, 1, input(ENGINE, TRANSFER), errors::ERR_INVALID_RECEIVER; "engine as receiver")]
    #[test_case(OWNER, 1, input(RECEIVER, ""), errors::ERR_INVALID_ACTIONS_NUMBER; "no actions")]
    #[test_case(OWNER, 1, b"not json".to_vec(), errors::ERR_JSON_DESERIALIZE.as_bytes(); "not json")]
    #[test_case(OWNER, 1, input(RECEIVER, r#""CreateAccount""#), errors::ERR_JSON_DESERIALIZE.as_bytes(); "unsupported action")]
    fn test_admin_call_rejects(predecessor: &str, deposit: u128, input: Vec<u8>, expected: &[u8]) {
        let storage = init_storage(input);

        assert_rejected(&storage, &call_env(predecessor, deposit), expected);
    }

    #[test]
    fn test_admin_call_checks_predecessor_not_signer() {
        // The owner signed the transaction, but the call reaches the engine from a contract.
        let storage = init_storage(input(RECEIVER, TRANSFER));
        let env = Fixed {
            signer_account_id: account(OWNER),
            ..call_env(STRANGER, 1)
        };

        assert_rejected(&storage, &env, errors::ERR_NOT_ALLOWED);
    }

    #[test]
    fn test_admin_call_requires_running_engine() {
        let storage = init_storage(input(RECEIVER, TRANSFER));
        let mut io = StoragePointer(&storage);
        let mut state = state::get_state(&io).unwrap();
        state.is_paused = true;
        state::set_state(&mut io, &state).unwrap();

        assert_rejected(&storage, &call_env(OWNER, 1), errors::ERR_PAUSED);
    }
}

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(alloc_error_handler))]
#![cfg_attr(
    all(feature = "log", target_arch = "wasm32"),
    feature(panic_info_message)
)]
#![deny(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::unreadable_literal
)]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
extern crate core;

mod map;
pub mod parameters {
    pub use aurora_engine_types::parameters::connector::*;
    pub use aurora_engine_types::parameters::engine::*;
}
pub mod proof {
    pub use aurora_engine_types::parameters::connector::Proof;
}
pub mod accounting;
pub mod admin_controlled;
#[cfg_attr(feature = "contract", allow(dead_code))]
pub mod connector;
pub mod deposit_event;
pub mod engine;
pub mod errors;
pub mod pausables;
mod prelude;
pub mod silo;
pub mod state;
pub mod xcc;

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[cfg(target_arch = "wasm32")]
#[panic_handler]
#[cfg_attr(not(feature = "log"), allow(unused_variables))]
#[no_mangle]
pub unsafe fn on_panic(info: &::core::panic::PanicInfo) -> ! {
    #[cfg(feature = "log")]
    {
        use prelude::ToString;

        if let Some(msg) = info.message() {
            let msg = if let Some(log) = info.location() {
                prelude::format!("{} [{}]", msg, log)
            } else {
                msg.to_string()
            };
            prelude::sdk::panic_utf8(msg.as_bytes());
        } else if let Some(log) = info.location() {
            prelude::sdk::panic_utf8(log.to_string().as_bytes());
        }
    }

    ::core::arch::wasm32::unreachable();
}

#[cfg(target_arch = "wasm32")]
#[alloc_error_handler]
#[no_mangle]
pub unsafe fn on_alloc_error(_: core::alloc::Layout) -> ! {
    ::core::arch::wasm32::unreachable();
}

#[cfg(feature = "contract")]
mod contract {
    use parameters::{SetOwnerArgs, SetUpgradeDelayBlocksArgs};
    use serde::de::DeserializeOwned;

    use crate::admin_controlled::AdminControlled;
    use crate::connector::{self, EthConnectorContract};
    use crate::engine::{self, Engine};
    use crate::parameters::{
        self, CallArgs, DeployErc20TokenArgs, GetErc20FromNep141CallArgs, GetStorageAtArgs,
        NEP141FtOnTransferArgs, NewCallArgs, PausePrecompilesCallArgs, SetContractDataCallArgs,
        StorageDepositCallArgs, StorageWithdrawCallArgs, SubmitArgs, TransferCallCallArgs,
        ViewCallArgs,
    };
    #[cfg(feature = "evm_bully")]
    use crate::parameters::{BeginBlockArgs, BeginChainArgs};
    use crate::pausables::{
        Authorizer, EnginePrecompilesPauser, PausedPrecompilesChecker, PausedPrecompilesManager,
        PrecompileFlags,
    };
    use crate::prelude::account_id::AccountId;
    use crate::prelude::parameters::RefundCallArgs;
    use crate::prelude::sdk::types::{
        near_account_to_evm_address, SdkExpect, SdkProcess, SdkUnwrap,
    };
    use crate::prelude::storage::{bytes_to_key, KeyPrefix};
    use crate::prelude::{
        sdk, u256_to_arr, vec, Address, PromiseResult, ToString, Vec, Yocto, ERR_FAILED_PARSE, H256,
    };
    use crate::{errors, pausables, silo, state};
    use aurora_engine_sdk::env::Env;
    use aurora_engine_sdk::io::{StorageIntermediate, IO};
    use aurora_engine_sdk::near_runtime::{Runtime, ViewEnv};
    use aurora_engine_sdk::promise::PromiseHandler;
    use aurora_engine_sdk::types::ExpectUtf8;
    use aurora_engine_types::borsh::BorshSerialize;
    use aurora_engine_types::parameters::connector::{
        EngineWithdrawCallArgs, SetEthConnectorContractAccountArgs, StorageUnregisterCallArgs,
    };
    use aurora_engine_types::parameters::engine::errors::ParseArgsError;
    use aurora_engine_types::parameters::engine::StorageUnregisterArgs;
    use aurora_engine_types::parameters::engine::{RelayerKeyArgs, RelayerKeyManagerArgs};
    use aurora_engine_types::parameters::silo::{
        FixedGasCostArgs, SiloParamsArgs, WhitelistArgs, WhitelistKindArgs, WhitelistStatusArgs,
    };
    use aurora_engine_types::parameters::{PromiseAction, PromiseBatchAction, WithdrawCallArgs};

    const CODE_KEY: &[u8; 4] = b"CODE";
    const CODE_STAGE_KEY: &[u8; 10] = b"CODE_STAGE";

    ///
    /// ADMINISTRATIVE METHODS
    ///

    /// Sets the configuration for the Engine.
    /// Should be called on deployment.
    #[no_mangle]
    pub extern "C" fn new() {
        let mut io = Runtime;

        if state::get_state(&io).is_ok() {
            sdk::panic_utf8(b"ERR_ALREADY_INITIALIZED");
        }

        let bytes = io.read_input().to_vec();
        let args = NewCallArgs::deserialize(&bytes).sdk_expect(errors::ERR_BORSH_DESERIALIZE);
        state::set_state(&mut io, &args.into()).sdk_unwrap();
    }

    /// Get version of the contract.
    #[no_mangle]
    pub extern "C" fn get_version() {
        let mut io = Runtime;
        let version = option_env!("NEAR_EVM_VERSION")
            .map_or(&include_bytes!("../../VERSION")[..], str::as_bytes);
        io.return_output(version);
    }

    /// Get owner account id for this contract.
    #[no_mangle]
    pub extern "C" fn get_owner() {
        let mut io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        io.return_output(state.owner_id.as_bytes());
    }

    /// Set owner account id for this contract.
    #[no_mangle]
    pub extern "C" fn set_owner() {
        let mut io = Runtime;
        let mut state = state::get_state(&io).sdk_unwrap();
        require_running(&state);
        require_owner_only(&state, &io.predecessor_account_id());

        let args: SetOwnerArgs = io.read_input_borsh().sdk_unwrap();
        if state.owner_id == args.new_owner {
            sdk::panic_utf8(errors::ERR_SAME_OWNER);
        } else {
            state.owner_id = args.new_owner;
            state::set_state(&mut io, &state).sdk_unwrap();
        }
    }

    /// Get bridge prover id for this contract.
    #[no_mangle]
    pub extern "C" fn get_bridge_prover() {
        let mut io = Runtime;
        let promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .get_bridge_prover();
        let promise_id = unsafe { io.promise_create_call(&promise_args) };
        io.promise_return(promise_id);
    }

    /// Get chain id for this contract.
    #[no_mangle]
    pub extern "C" fn get_chain_id() {
        let mut io = Runtime;
        io.return_output(&state::get_state(&io).sdk_unwrap().chain_id);
    }

    #[no_mangle]
    pub extern "C" fn get_upgrade_delay_blocks() {
        let mut io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        io.return_output(&state.upgrade_delay_blocks.to_le_bytes());
    }

    #[no_mangle]
    pub extern "C" fn set_upgrade_delay_blocks() {
        let mut io = Runtime;
        let mut state = state::get_state(&io).sdk_unwrap();
        require_running(&state);
        require_owner_only(&state, &io.predecessor_account_id());
        let args: SetUpgradeDelayBlocksArgs = io.read_input_borsh().sdk_unwrap();
        state.upgrade_delay_blocks = args.upgrade_delay_blocks;
        state::set_state(&mut io, &state).sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn get_upgrade_index() {
        let mut io = Runtime;
        let index = internal_get_upgrade_index();
        io.return_output(&index.to_le_bytes());
    }

    /// Stage new code for deployment.
    #[no_mangle]
    pub extern "C" fn stage_upgrade() {
        let mut io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        require_running(&state);
        let delay_block_height = io.block_height() + state.upgrade_delay_blocks;
        require_owner_only(&state, &io.predecessor_account_id());
        io.read_input_and_store(&bytes_to_key(KeyPrefix::Config, CODE_KEY));
        io.write_storage(
            &bytes_to_key(KeyPrefix::Config, CODE_STAGE_KEY),
            &delay_block_height.to_le_bytes(),
        );
    }

    /// Deploy staged upgrade.
    #[no_mangle]
    pub extern "C" fn deploy_upgrade() {
        let mut io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        require_running(&state);
        let index = internal_get_upgrade_index();
        if io.block_height() <= index {
            sdk::panic_utf8(errors::ERR_NOT_ALLOWED_TOO_EARLY);
        }
        Runtime::self_deploy(&bytes_to_key(KeyPrefix::Config, CODE_KEY));
        io.remove_storage(&bytes_to_key(KeyPrefix::Config, CODE_STAGE_KEY));
    }

    /// Called as part of the upgrade process (see `engine-sdk::self_deploy`). This function is meant
    /// to make any necessary changes to the state such that it aligns with the newly deployed
    /// code.
    #[no_mangle]
    #[allow(clippy::missing_const_for_fn)]
    pub extern "C" fn state_migration() {
        // TODO: currently we don't have migrations
    }

    /// Resumes previously [`paused`] precompiles.
    ///
    /// [`paused`]: crate::contract::pause_precompiles
    #[no_mangle]
    pub extern "C" fn resume_precompiles() {
        let io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        require_running(&state);
        let predecessor_account_id = io.predecessor_account_id();

        require_owner_only(&state, &predecessor_account_id);

        let args: PausePrecompilesCallArgs = io.read_input_borsh().sdk_unwrap();
        let flags = PrecompileFlags::from_bits_truncate(args.paused_mask);
        let mut pauser = EnginePrecompilesPauser::from_io(io);
        pauser.resume_precompiles(flags);
    }

    /// Pauses a precompile.
    #[no_mangle]
    pub extern "C" fn pause_precompiles() {
        let io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        let authorizer: pausables::EngineAuthorizer = engine::get_authorizer(&io);

        if !authorizer.is_authorized(&io.predecessor_account_id()) {
            sdk::panic_utf8(errors::ERR_NOT_ALLOWED);
        }

        let args: PausePrecompilesCallArgs = io.read_input_borsh().sdk_unwrap();
        let flags = PrecompileFlags::from_bits_truncate(args.paused_mask);
        let mut pauser = EnginePrecompilesPauser::from_io(io);
        pauser.pause_precompiles(flags);
    }

    /// Returns an unsigned integer where each 1-bit means that a precompile corresponding to that bit is paused and
    /// 0-bit means not paused.
    #[no_mangle]
    pub extern "C" fn paused_precompiles() {
        let mut io = Runtime;
        let pauser = EnginePrecompilesPauser::from_io(io);
        let data = pauser.paused().bits().to_le_bytes();
        io.return_output(&data);
    }

    /// Sets the flag to pause the contract.
    #[no_mangle]
    pub extern "C" fn pause_contract() {
        let mut io = Runtime;
        let mut state = state::get_state(&io).sdk_unwrap();
        require_owner_only(&state, &io.predecessor_account_id());
        if state.is_paused {
            sdk::panic_utf8(errors::ERR_PAUSED);
        }
        state.is_paused = true;
        state::set_state(&mut io, &state).sdk_unwrap();
    }

    /// Sets the flag to resume the contract.
    #[no_mangle]
    pub extern "C" fn resume_contract() {
        let mut io = Runtime;
        let mut state = state::get_state(&io).sdk_unwrap();
        require_owner_only(&state, &io.predecessor_account_id());
        if !state.is_paused {
            sdk::panic_utf8(errors::ERR_RUNNING);
        }
        state.is_paused = false;
        state::set_state(&mut io, &state).sdk_unwrap();
    }

    ///
    /// MUTATIVE METHODS
    ///

    /// Deploy code into the EVM.
    #[no_mangle]
    pub extern "C" fn deploy_code() {
        let io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        let input = io.read_input().to_vec();
        let current_account_id = io.current_account_id();
        let mut engine: Engine<_, _> = Engine::new(
            predecessor_address(&io.predecessor_account_id()),
            current_account_id,
            io,
            &io,
        )
        .sdk_unwrap();
        Engine::deploy_code_with_input(&mut engine, input, &mut Runtime)
            .map(|res| res.try_to_vec().sdk_expect(errors::ERR_SERIALIZE))
            .sdk_process();
        // TODO: charge for storage
    }

    /// Call method on the EVM contract.
    #[no_mangle]
    pub extern "C" fn call() {
        let io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        let bytes = io.read_input().to_vec();
        let args = CallArgs::deserialize(&bytes).sdk_expect(errors::ERR_BORSH_DESERIALIZE);
        let current_account_id = io.current_account_id();
        let predecessor_account_id = io.predecessor_account_id();

        // During the XCC flow the Engine will call itself to move wNEAR
        // to the user's sub-account. We do not want this move to happen
        // if prior promises in the flow have failed.
        if current_account_id == predecessor_account_id {
            let check_promise: Result<(), &[u8]> = match io.promise_result_check() {
                Some(true) | None => Ok(()),
                Some(false) => Err(b"ERR_CALLBACK_OF_FAILED_PROMISE"),
            };
            check_promise.sdk_unwrap();
        }

        let mut engine: Engine<_, _> = Engine::new(
            predecessor_address(&predecessor_account_id),
            current_account_id,
            io,
            &io,
        )
        .sdk_unwrap();
        Engine::call_with_args(&mut engine, args, &mut Runtime)
            .map(|res| res.try_to_vec().sdk_expect(errors::ERR_SERIALIZE))
            .sdk_process();
        // TODO: charge for storage
    }

    /// Process signed Ethereum transaction.
    /// Must match `CHAIN_ID` to make sure it's signed for given chain vs replayed from another chain.
    #[no_mangle]
    pub extern "C" fn submit() {
        let io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        require_running(&state);
        let tx_data = io.read_input().to_vec();
        let current_account_id = io.current_account_id();
        let relayer_address = predecessor_address(&io.predecessor_account_id());
        let args = SubmitArgs {
            tx_data,
            ..Default::default()
        };
        let result = engine::submit(
            io,
            &io,
            &args,
            state,
            current_account_id,
            relayer_address,
            &mut Runtime,
        );

        result
            .map(|res| res.try_to_vec().sdk_expect(errors::ERR_SERIALIZE))
            .sdk_process();
    }

    /// Analog of the `submit` function, but waits for the `SubmitArgs` structure rather than
    /// the array of bytes representing the transaction.
    #[no_mangle]
    pub extern "C" fn submit_with_args() {
        let io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        require_running(&state);
        let args: SubmitArgs = io.read_input_borsh().sdk_unwrap();
        let current_account_id = io.current_account_id();
        let relayer_address = predecessor_address(&io.predecessor_account_id());
        let result = engine::submit(
            io,
            &io,
            &args,
            state,
            current_account_id,
            relayer_address,
            &mut Runtime,
        );

        result
            .map(|res| res.try_to_vec().sdk_expect(errors::ERR_SERIALIZE))
            .sdk_process();
    }

    #[no_mangle]
    pub extern "C" fn register_relayer() {
        let io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        let relayer_address = io.read_input_arr20().sdk_unwrap();

        let current_account_id = io.current_account_id();
        let predecessor_account_id = io.predecessor_account_id();
        let mut engine: Engine<_, _> = Engine::new(
            predecessor_address(&predecessor_account_id),
            current_account_id,
            io,
            &io,
        )
        .sdk_unwrap();
        engine.register_relayer(
            predecessor_account_id.as_bytes(),
            Address::from_array(relayer_address),
        );
    }

    /// Updates the bytecode for user's router contracts created by the engine.
    /// These contracts are where cross-contract calls initiated by the EVM precompile
    /// will be sent from.
    #[no_mangle]
    pub extern "C" fn factory_update() {
        let mut io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        require_running(&state);
        require_owner_only(&state, &io.predecessor_account_id());
        let bytes = io.read_input().to_vec();
        let router_bytecode = crate::xcc::RouterCode::new(bytes);
        crate::xcc::update_router_code(&mut io, &router_bytecode);
    }

    /// Updates the bytecode version for the given account. This is only called as a callback
    /// when a new version of the router contract is deployed to an account.
    #[no_mangle]
    pub extern "C" fn factory_update_address_version() {
        let mut io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        // The function is only set to be private, otherwise callback error will happen.
        io.assert_private_call().sdk_unwrap();
        let check_deploy: Result<(), &[u8]> = match io.promise_result_check() {
            Some(true) => Ok(()),
            Some(false) => Err(b"ERR_ROUTER_DEPLOY_FAILED"),
            None => Err(b"ERR_ROUTER_UPDATE_NOT_CALLBACK"),
        };
        check_deploy.sdk_unwrap();
        let args: crate::xcc::AddressVersionUpdateArgs = io.read_input_borsh().sdk_unwrap();
        crate::xcc::set_code_version_of_address(&mut io, &args.address, args.version);
    }

    /// Sets the address for the `wNEAR` ERC-20 contract. This contract will be used by the
    /// cross-contract calls feature to have users pay for their NEAR transactions.
    #[no_mangle]
    pub extern "C" fn factory_set_wnear_address() {
        let mut io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        require_running(&state);
        require_owner_only(&state, &io.predecessor_account_id());
        let address = io.read_input_arr20().sdk_unwrap();
        crate::xcc::set_wnear_address(&mut io, &Address::from_array(address));
    }

    /// Returns the address for the `wNEAR` ERC-20 contract in borsh format.
    #[no_mangle]
    pub extern "C" fn factory_get_wnear_address() {
        let mut io = Runtime;
        let address = aurora_engine_precompiles::xcc::state::get_wnear_address(&io);
        let bytes = address.try_to_vec().sdk_expect(errors::ERR_SERIALIZE);
        io.return_output(&bytes);
    }

    /// Create and/or fund an XCC sub-account directly (as opposed to having one be automatically
    /// created via the XCC precompile in the EVM). The purpose of this method is to enable
    /// XCC on engine instances where wrapped NEAR (WNEAR) is not bridged.
    #[no_mangle]
    pub extern "C" fn fund_xcc_sub_account() {
        let io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        require_running(&state);
        // This method can only be called by the owner because it allows specifying the
        // account ID of the wNEAR account. This information must be accurate for the
        // sub-account to work properly, therefore this method can only be called by
        // a trusted user.
        require_owner_only(&state, &io.predecessor_account_id());
        let args: crate::xcc::FundXccArgs = io.read_input_borsh().sdk_unwrap();
        crate::xcc::fund_xcc_sub_account(&io, &mut Runtime, &io, args).sdk_unwrap();
    }

    /// Deploy ERC20 token mapped to a NEP141
    #[no_mangle]
    pub extern "C" fn deploy_erc20_token() {
        let mut io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        // Id of the NEP141 token in Near
        let args: DeployErc20TokenArgs = io.read_input_borsh().sdk_unwrap();
        let address = engine::deploy_erc20_token(args, io, &io, &mut Runtime).sdk_unwrap();

        io.return_output(
            &address
                .as_bytes()
                .try_to_vec()
                .sdk_expect(errors::ERR_SERIALIZE),
        );

        // TODO: charge for storage
    }

    /// Callback invoked by exit to NEAR precompile to handle potential
    /// errors in the exit call.
    #[no_mangle]
    pub extern "C" fn refund_on_error() {
        let io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        require_running(&state);
        io.assert_private_call().sdk_unwrap();

        // This function should only be called as the callback of
        // exactly one promise.
        if io.promise_results_count() != 1 {
            sdk::panic_utf8(errors::ERR_PROMISE_COUNT);
        }

        if let Some(PromiseResult::Successful(_)) = io.promise_result(0) {
            // Promise succeeded -- nothing to do
        } else {
            // Exit call failed; need to refund tokens
            let args: RefundCallArgs = io.read_input_borsh().sdk_unwrap();
            let refund_result =
                engine::refund_on_error(io, &io, state, &args, &mut Runtime).sdk_unwrap();

            if !refund_result.status.is_ok() {
                sdk::panic_utf8(errors::ERR_REFUND_FAILURE);
            }
        }
    }

    /// Sets relayer key manager.
    #[no_mangle]
    pub extern "C" fn set_key_manager() {
        let mut io = Runtime;
        let mut state = state::get_state(&io).sdk_unwrap();

        require_owner_only(&state, &io.predecessor_account_id());

        let key_manager =
            serde_json::from_slice::<RelayerKeyManagerArgs>(&io.read_input().to_vec())
                .map(|args| args.key_manager)
                .sdk_expect(errors::ERR_JSON_DESERIALIZE);

        if state.key_manager == key_manager {
            sdk::panic_utf8(errors::ERR_SAME_KEY_MANAGER)
        } else {
            state.key_manager = key_manager;
            state::set_state(&mut io, &state).sdk_unwrap();
        }
    }

    /// Adds a relayer function call key.
    #[no_mangle]
    pub extern "C" fn add_relayer_key() {
        let mut io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        require_key_manager_only(&state, &io.predecessor_account_id());

        let public_key = serde_json::from_slice::<RelayerKeyArgs>(&io.read_input().to_vec())
            .map(|args| args.public_key)
            .sdk_expect(errors::ERR_JSON_DESERIALIZE);
        let allowance = Yocto::new(io.attached_deposit());
        sdk::log!("attached key allowance: {allowance}");

        if allowance.as_u128() < 100 {
            // TODO: Clarify the minimum amount if check is needed then change error type
            sdk::panic_utf8(errors::ERR_NOT_ALLOWED);
        }

        engine::add_function_call_key(&mut io, &public_key);

        let action = PromiseAction::AddFunctionCallKey {
            public_key,
            allowance,
            nonce: 0, // not actually used - depends on block height
            receiver_id: io.current_account_id(),
            function_names: "call,submit,submit_with_args".to_string(),
        };
        let promise = PromiseBatchAction {
            target_account_id: io.current_account_id(),
            actions: vec![action],
        };

        let promise_id = unsafe { io.promise_create_batch(&promise) };
        io.promise_return(promise_id);
    }

    /// Removes a relayer function call key.
    #[no_mangle]
    pub extern "C" fn remove_relayer_key() {
        let mut io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        require_key_manager_only(&state, &io.predecessor_account_id());

        let args: RelayerKeyArgs = serde_json::from_slice(&io.read_input().to_vec())
            .sdk_expect(errors::ERR_JSON_DESERIALIZE);

        engine::remove_function_call_key(&mut io, &args.public_key).sdk_unwrap();

        let action = PromiseAction::DeleteKey {
            public_key: args.public_key,
        };
        let promise = PromiseBatchAction {
            target_account_id: io.current_account_id(),
            actions: vec![action],
        };

        let promise_id = unsafe { io.promise_create_batch(&promise) };
        io.promise_return(promise_id);
    }

    ///
    /// NONMUTATIVE METHODS
    ///
    #[no_mangle]
    pub extern "C" fn view() {
        let mut io = Runtime;
        let env = ViewEnv;
        let args: ViewCallArgs = io.read_input_borsh().sdk_unwrap();
        let current_account_id = io.current_account_id();
        let engine: Engine<_, _> =
            Engine::new(args.sender, current_account_id, io, &env).sdk_unwrap();
        let result = Engine::view_with_args(&engine, args).sdk_unwrap();
        io.return_output(&result.try_to_vec().sdk_expect(errors::ERR_SERIALIZE));
    }

    #[no_mangle]
    pub extern "C" fn get_block_hash() {
        let mut io = Runtime;
        let block_height = io.read_input_borsh().sdk_unwrap();
        let account_id = io.current_account_id();
        let chain_id = state::get_state(&io)
            .map(|state| state.chain_id)
            .sdk_unwrap();
        let block_hash = engine::compute_block_hash(chain_id, block_height, account_id.as_bytes());
        io.return_output(block_hash.as_bytes());
    }

    #[no_mangle]
    pub extern "C" fn get_code() {
        let mut io = Runtime;
        let address = io.read_input_arr20().sdk_unwrap();
        let code = engine::get_code(&io, &Address::from_array(address));
        io.return_output(&code);
    }

    #[no_mangle]
    pub extern "C" fn get_balance() {
        let mut io = Runtime;
        let address = io.read_input_arr20().sdk_unwrap();
        let balance = engine::get_balance(&io, &Address::from_array(address));
        io.return_output(&balance.to_bytes());
    }

    #[no_mangle]
    pub extern "C" fn get_nonce() {
        let mut io = Runtime;
        let address = io.read_input_arr20().sdk_unwrap();
        let nonce = engine::get_nonce(&io, &Address::from_array(address));
        io.return_output(&u256_to_arr(&nonce));
    }

    #[no_mangle]
    pub extern "C" fn get_storage_at() {
        let mut io = Runtime;
        let args: GetStorageAtArgs = io.read_input_borsh().sdk_unwrap();
        let address = args.address;
        let generation = engine::get_generation(&io, &address);
        let value = engine::get_storage(&io, &args.address, &H256(args.key), generation);
        io.return_output(&value.0);
    }

    ///
    /// BENCHMARKING METHODS
    ///
    #[cfg(feature = "evm_bully")]
    #[no_mangle]
    pub extern "C" fn begin_chain() {
        use crate::prelude::U256;
        let mut io = Runtime;
        let mut state = state::get_state(&io).sdk_unwrap();
        require_owner_only(&state, &io.predecessor_account_id());
        let args: BeginChainArgs = io.read_input_borsh().sdk_unwrap();
        state.chain_id = args.chain_id;
        state::set_state(&mut io, &state).sdk_unwrap();
        // set genesis block balances
        for account_balance in args.genesis_alloc {
            engine::set_balance(
                &mut io,
                &account_balance.address,
                &crate::prelude::Wei::new(U256::from(account_balance.balance)),
            );
        }
        // return new chain ID
        io.return_output(&state::get_state(&io).sdk_unwrap().chain_id);
    }

    #[cfg(feature = "evm_bully")]
    #[no_mangle]
    pub extern "C" fn begin_block() {
        let io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        require_owner_only(&state, &io.predecessor_account_id());
        let _args: BeginBlockArgs = io.read_input_borsh().sdk_unwrap();
        // TODO: https://github.com/aurora-is-near/aurora-engine/issues/2
    }

    ///
    /// ETH-CONNECTOR
    ///
    #[no_mangle]
    pub extern "C" fn new_eth_connector() {
        let io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        require_running(&state);
        // Only the owner can initialize the EthConnector
        let is_private = io.assert_private_call();

        if is_private.is_err() {
            require_owner_only(&state, &io.predecessor_account_id());
        }
    }

    #[no_mangle]
    pub extern "C" fn set_eth_connector_contract_data() {
        let mut io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        require_running(&state);
        // Only the owner can set the EthConnector contract data
        let is_private = io.assert_private_call();
        if is_private.is_err() {
            require_owner_only(&state, &io.predecessor_account_id());
        }

        let args: SetContractDataCallArgs = io.read_input_borsh().sdk_unwrap();
        connector::set_contract_data(&mut io, args).sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn withdraw() {
        let mut io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        io.assert_one_yocto().sdk_unwrap();
        let args: WithdrawCallArgs = io.read_input_borsh().sdk_unwrap();
        let input = EngineWithdrawCallArgs {
            sender_id: io.predecessor_account_id(),
            recipient_address: args.recipient_address,
            amount: args.amount,
        }
        .try_to_vec()
        .unwrap();

        let promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .withdraw_eth_from_near(input);
        let promise_id = unsafe { io.promise_create_call(&promise_args) };
        io.promise_return(promise_id);
    }

    #[no_mangle]
    pub extern "C" fn deposit() {
        let mut io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        let input = io.read_input().to_vec();
        let promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .deposit(input);
        let promise_id = unsafe { io.promise_create_call(&promise_args) };
        io.promise_return(promise_id);
    }

    #[no_mangle]
    pub extern "C" fn is_used_proof() {
        let mut io = Runtime;
        let input = io.read_input().to_vec();
        let promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .is_used_proof(input);
        let promise_id = unsafe { io.promise_create_call(&promise_args) };
        io.promise_return(promise_id);
    }

    #[no_mangle]
    pub extern "C" fn ft_total_supply() {
        let mut io = Runtime;
        let promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .ft_total_eth_supply_on_near();
        let promise_id = unsafe { io.promise_create_call(&promise_args) };
        io.promise_return(promise_id);
    }

    #[no_mangle]
    pub extern "C" fn ft_total_eth_supply_on_near() {
        let mut io = Runtime;
        let promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .ft_total_eth_supply_on_near();
        let promise_id = unsafe { io.promise_create_call(&promise_args) };
        io.promise_return(promise_id);
    }

    #[no_mangle]
    pub extern "C" fn ft_balance_of() {
        let mut io = Runtime;
        let input = io.read_input().to_vec();
        let promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .ft_balance_of(input);
        let promise_id = unsafe { io.promise_create_call(&promise_args) };
        io.promise_return(promise_id);
    }

    #[no_mangle]
    pub extern "C" fn ft_balance_of_eth() {
        let io = Runtime;
        let args = io.read_input_borsh().sdk_unwrap();
        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .ft_balance_of_eth_on_aurora(&args)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn ft_transfer() {
        use crate::parameters::TransferCallArgs;
        let mut io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        io.assert_one_yocto().sdk_unwrap();
        let input = read_json_args(&io)
            .and_then(|args: TransferCallArgs| {
                serde_json::to_vec(&(
                    io.predecessor_account_id(),
                    args.receiver_id,
                    args.amount,
                    args.memo,
                ))
            })
            .map_err(Into::<ParseTypeFromJsonError>::into)
            .sdk_unwrap();

        let promise_arg = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .ft_transfer(input);
        let promise_id = unsafe { io.promise_create_call(&promise_arg) };
        io.promise_return(promise_id);
    }

    #[no_mangle]
    pub extern "C" fn ft_transfer_call() {
        let mut io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        // Check is payable
        io.assert_one_yocto().sdk_unwrap();
        let input = read_json_args(&io)
            .and_then(|args: TransferCallCallArgs| {
                serde_json::to_vec(&(
                    io.predecessor_account_id(),
                    args.receiver_id,
                    args.amount,
                    args.memo,
                    args.msg,
                ))
            })
            .map_err(Into::<ParseTypeFromJsonError>::into)
            .sdk_unwrap();

        let promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .ft_transfer_call(input);
        let promise_id = unsafe { io.promise_create_call(&promise_args) };
        io.promise_return(promise_id);
    }

    /// Allow receiving NEP141 tokens to the EVM contract.
    ///
    /// This function returns the amount of tokens to return to the sender.
    /// Either all tokens are transferred tokens are returned in case of an
    /// error, or no token is returned if tx was successful.
    #[no_mangle]
    pub extern "C" fn ft_on_transfer() {
        let io = Runtime;
        let current_account_id = io.current_account_id();
        let predecessor_account_id = io.predecessor_account_id();
        let mut engine: Engine<_, _> = Engine::new(
            predecessor_address(&predecessor_account_id),
            current_account_id.clone(),
            io,
            &io,
        )
        .sdk_unwrap();

        let args: NEP141FtOnTransferArgs = serde_json::from_slice(&io.read_input().to_vec())
            .map_err(Into::<ParseTypeFromJsonError>::into)
            .sdk_unwrap();
        let mut eth_connector = EthConnectorContract::init_instance(io).sdk_unwrap();

        if predecessor_account_id == eth_connector.get_eth_connector_contract_account() {
            eth_connector.ft_on_transfer(&engine, &args).sdk_unwrap();
        } else {
            engine.receive_erc20_tokens(
                &predecessor_account_id,
                &args,
                &current_account_id,
                &mut Runtime,
            );
        }
    }

    #[no_mangle]
    pub extern "C" fn storage_deposit() {
        let mut io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        let input = read_json_args(&io)
            .and_then(|args: StorageDepositCallArgs| {
                serde_json::to_vec(&(
                    io.predecessor_account_id(),
                    args.account_id,
                    args.registration_only,
                ))
            })
            .map_err(Into::<ParseTypeFromJsonError>::into)
            .sdk_unwrap();

        let promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .storage_deposit(input, io.attached_deposit());
        let promise_id = unsafe { io.promise_create_call(&promise_args) };

        io.promise_return(promise_id);
    }

    #[no_mangle]
    pub extern "C" fn storage_unregister() {
        let mut io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        io.assert_one_yocto().sdk_unwrap();

        let input = read_json_args(&io)
            .and_then(|args: StorageUnregisterCallArgs| {
                serde_json::to_vec(&(io.predecessor_account_id(), args.force))
            })
            .map_err(Into::<ParseTypeFromJsonError>::into)
            .sdk_unwrap();

        let promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .storage_unregister(input);
        let promise_id = unsafe { io.promise_create_call(&promise_args) };

        io.promise_return(promise_id);
    }

    #[no_mangle]
    pub extern "C" fn storage_withdraw() {
        let mut io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        io.assert_one_yocto().sdk_unwrap();

        let input = read_json_args(&io)
            .and_then(|args: StorageWithdrawCallArgs| serde_json::to_vec(&(io.predecessor_account_id(), args.amount)))
            .map_err(Into::<ParseTypeFromJsonError>::into)
            .sdk_unwrap();

        let promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .storage_withdraw(input);
        let promise_id = unsafe { io.promise_create_call(&promise_args) };

        io.promise_return(promise_id);
    }

    #[no_mangle]
    pub extern "C" fn storage_balance_of() {
        let mut io = Runtime;
        let input = io.read_input().to_vec();
        let promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .storage_balance_of(input);
        let promise_id = unsafe { io.promise_create_call(&promise_args) };
        io.promise_return(promise_id);
    }

    #[no_mangle]
    pub extern "C" fn get_eth_connector_contract_account() {
        let mut io = Runtime;
        let account = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .get_eth_connector_contract_account();
        let data = account.try_to_vec().expect(ERR_FAILED_PARSE);
        io.return_output(&data);
    }

    #[no_mangle]
    pub extern "C" fn set_eth_connector_contract_account() {
        let io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        require_running(&state);
        let is_private = io.assert_private_call();

        if is_private.is_err() {
            require_owner_only(&state, &io.predecessor_account_id());
        }

        let args: SetEthConnectorContractAccountArgs = io.read_input_borsh().sdk_unwrap();

        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .set_eth_connector_contract_account(&args.account);
    }

    #[no_mangle]
    pub extern "C" fn disable_legacy_nep141() {
        let io = Runtime;
        let state = state::get_state(&io).sdk_unwrap();
        require_owner_only(&state, &io.predecessor_account_id());

        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .disable_legacy_nep141();
    }

    #[no_mangle]
    pub extern "C" fn get_paused_flags() {
        let mut io = Runtime;
        let promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .get_paused_flags();
        let promise_id = unsafe { io.promise_create_call(&promise_args) };
        io.promise_return(promise_id);
    }

    #[no_mangle]
    pub extern "C" fn ft_metadata() {
        let mut io = Runtime;
        let promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .get_metadata();
        let promise_id = unsafe { io.promise_create_call(&promise_args) };
        io.promise_return(promise_id);
    }

    #[cfg(feature = "integration-test")]
    #[no_mangle]
    pub extern "C" fn verify_log_entry() {
        sdk::log!("Call from verify_log_entry");
        let mut io = Runtime;
        let data = true.try_to_vec().unwrap();
        io.return_output(&data);
    }

    /// Function used to create accounts for tests
    #[cfg(feature = "integration-test")]
    #[no_mangle]
    pub extern "C" fn mint_account() {
        use crate::prelude::{NEP141Wei, U256};
        use evm::backend::ApplyBackend;

        let io = Runtime;
        let args: ([u8; 20], u64, u64) = io.read_input_borsh().sdk_expect(errors::ERR_ARGS);
        let address = Address::from_array(args.0);
        let nonce = U256::from(args.1);
        let balance = NEP141Wei::new(u128::from(args.2));
        let current_account_id = io.current_account_id();
        let mut engine: Engine<_, _> =
            Engine::new(address, current_account_id, io, &io).sdk_unwrap();
        let state_change = evm::backend::Apply::Modify {
            address: address.raw(),
            basic: evm::backend::Basic {
                balance: U256::from(balance.as_u128()),
                nonce,
            },
            code: None,
            storage: core::iter::empty(),
            reset_storage: false,
        };
        engine.apply(core::iter::once(state_change), core::iter::empty(), false);
    }

    #[no_mangle]
    pub extern "C" fn get_erc20_from_nep141() {
        let mut io = Runtime;
        let args: GetErc20FromNep141CallArgs = io.read_input_borsh().sdk_unwrap();

        io.return_output(
            engine::get_erc20_from_nep141(&io, &args.nep141)
                .sdk_unwrap()
                .as_slice(),
        );
    }

    #[no_mangle]
    pub extern "C" fn get_nep141_from_erc20() {
        let mut io = Runtime;
        let erc20_address: crate::engine::ERC20Address =
            io.read_input().to_vec().try_into().sdk_unwrap();
        io.return_output(
            engine::nep141_erc20_map(io)
                .lookup_right(&erc20_address)
                .sdk_expect("ERC20_NOT_FOUND")
                .as_ref(),
        );
    }

    ///
    /// Silo
    ///
    #[no_mangle]
    pub extern "C" fn get_fixed_gas_cost() {
        let mut io = Runtime;
        let cost = FixedGasCostArgs {
            cost: silo::get_fixed_gas_cost(&io),
        };

        io.return_output(&cost.try_to_vec().map_err(|e| e.to_string()).sdk_unwrap());
    }

    #[no_mangle]
    pub extern "C" fn set_fixed_gas_cost() {
        let mut io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        silo::assert_admin(&io).sdk_unwrap();
        let args: FixedGasCostArgs = io.read_input_borsh().sdk_unwrap();
        args.cost.sdk_expect("FIXED_GAS_COST_IS_NONE"); // Use `set_silo_params` to disable the silo mode.
        silo::get_silo_params(&io).sdk_expect("SILO_MODE_IS_OFF"); // Use `set_silo_params` to enable the silo mode.
        silo::set_fixed_gas_cost(&mut io, args.cost);
    }

    #[no_mangle]
    pub extern "C" fn get_silo_params() {
        let mut io = Runtime;
        let params = silo::get_silo_params(&io);

        io.return_output(&params.try_to_vec().map_err(|e| e.to_string()).sdk_unwrap());
    }

    #[no_mangle]
    pub extern "C" fn set_silo_params() {
        let mut io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        silo::assert_admin(&io).sdk_unwrap();
        let args: Option<SiloParamsArgs> = io.read_input_borsh().sdk_unwrap();
        silo::set_silo_params(&mut io, args);
    }

    #[no_mangle]
    pub extern "C" fn set_whitelist_status() {
        let io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        silo::assert_admin(&io).sdk_unwrap();
        let args: WhitelistStatusArgs = io.read_input_borsh().sdk_unwrap();
        silo::set_whitelist_status(&io, &args);
    }

    #[no_mangle]
    pub extern "C" fn get_whitelist_status() {
        let mut io = Runtime;
        let args: WhitelistKindArgs = io.read_input_borsh().sdk_unwrap();
        let status = silo::get_whitelist_status(&io, &args)
            .try_to_vec()
            .map_err(|e| e.to_string())
            .sdk_unwrap();

        io.return_output(&status);
    }

    #[no_mangle]
    pub extern "C" fn add_entry_to_whitelist() {
        let io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        silo::assert_admin(&io).sdk_unwrap();

        let args: WhitelistArgs = io.read_input_borsh().sdk_unwrap();
        silo::add_entry_to_whitelist(&io, &args);
    }

    #[no_mangle]
    pub extern "C" fn add_entry_to_whitelist_batch() {
        let io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        silo::assert_admin(&io).sdk_unwrap();

        let args: Vec<WhitelistArgs> = io.read_input_borsh().sdk_unwrap();
        silo::add_entry_to_whitelist_batch(&io, args);
    }

    #[no_mangle]
    pub extern "C" fn remove_entry_from_whitelist() {
        let io = Runtime;
        require_running(&state::get_state(&io).sdk_unwrap());
        silo::assert_admin(&io).sdk_unwrap();

        let args: WhitelistArgs = io.read_input_borsh().sdk_unwrap();
        silo::remove_entry_from_whitelist(&io, &args);
    }

    ///
    /// Utility methods.
    ///

    fn internal_get_upgrade_index() -> u64 {
        let io = Runtime;
        match io.read_u64(&bytes_to_key(KeyPrefix::Config, CODE_STAGE_KEY)) {
            Ok(index) => index,
            Err(sdk::error::ReadU64Error::InvalidU64) => {
                sdk::panic_utf8(errors::ERR_INVALID_UPGRADE)
            }
            Err(sdk::error::ReadU64Error::MissingValue) => sdk::panic_utf8(errors::ERR_NO_UPGRADE),
        }
    }

    fn require_owner_only(state: &state::EngineState, predecessor_account_id: &AccountId) {
        if &state.owner_id != predecessor_account_id {
            sdk::panic_utf8(errors::ERR_NOT_OWNER);
        }
    }

    fn require_running(state: &state::EngineState) {
        if state.is_paused {
            sdk::panic_utf8(errors::ERR_PAUSED);
        }
    }

    fn require_key_manager_only(state: &state::EngineState, predecessor_account_id: &AccountId) {
        let key_manager = state
            .key_manager
            .as_ref()
            .expect_utf8(errors::ERR_KEY_MANAGER_IS_NOT_SET);
        if key_manager != predecessor_account_id {
            sdk::panic_utf8(errors::ERR_NOT_ALLOWED);
        }
    }

    fn predecessor_address(predecessor_account_id: &AccountId) -> Address {
        near_account_to_evm_address(predecessor_account_id.as_bytes())
    }

    fn read_json_args<I: IO, T>(io: &I) -> Result<T, ParseArgsError>
    where
        T: DeserializeOwned,
    {
        let bytes = io.read_input().to_vec();
        aurora_engine_types::parameters::engine::parse_json_args(&bytes)
    }

    mod exports {
        extern "C" {
            pub(crate) fn value_return(value_len: u64, value_ptr: u64);
        }
    }
}

pub trait AuroraState {
    fn add_promise(&mut self, promise: aurora_engine_types::parameters::PromiseCreateArgs);
}

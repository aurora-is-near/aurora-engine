#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(alloc_error_handler))]
#![cfg_attr(
    all(feature = "log", target_arch = "wasm32"),
    feature(panic_info_message)
)]
#![deny(clippy::as_conversions)]

use aurora_engine_types::parameters::PromiseCreateArgs;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
extern crate core;

mod map;
pub mod parameters;
pub mod proof;

pub mod accounting;
pub mod admin_controlled;
#[cfg_attr(feature = "contract", allow(dead_code))]
pub mod connector;
pub mod deposit_event;
pub mod engine;
pub mod errors;
pub mod fungible_token;
pub mod json;
pub mod log_entry;
pub mod pausables;
mod prelude;
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
    use borsh::{BorshDeserialize, BorshSerialize};

    use crate::connector::{self, EthConnectorContract};
    use crate::engine::{self, Engine, EngineState};
    use crate::fungible_token::FungibleTokenMetadata;
    use crate::json::parse_json;
    use crate::parameters::{
        self, CallArgs, DeployErc20TokenArgs, GetErc20FromNep141CallArgs, GetStorageAtArgs,
        InitCallArgs, IsUsedProofCallArgs, NEP141FtOnTransferArgs, NewCallArgs,
        PauseEthConnectorCallArgs, PausePrecompilesCallArgs, ResolveTransferCallArgs,
        SetContractDataCallArgs, StorageDepositCallArgs, StorageWithdrawCallArgs,
        TransferCallCallArgs, ViewCallArgs,
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
        sdk, u256_to_arr, Address, PromiseResult, ToString, Yocto, ERR_FAILED_PARSE, H256,
    };
    use crate::{errors, pausables};
    use aurora_engine_sdk::env::Env;
    use aurora_engine_sdk::io::{StorageIntermediate, IO};
    use aurora_engine_sdk::near_runtime::{Runtime, ViewEnv};
    use aurora_engine_sdk::promise::PromiseHandler;

    #[cfg(feature = "integration-test")]
    use crate::prelude::NearGas;

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
        if let Ok(state) = engine::get_state(&io) {
            require_owner_only(&state, &io.predecessor_account_id());
        }

        let args: NewCallArgs = io.read_input_borsh().sdk_unwrap();
        engine::set_state(&mut io, args.into());
    }

    /// Get version of the contract.
    #[no_mangle]
    pub extern "C" fn get_version() {
        let mut io = Runtime;
        let version = match option_env!("NEAR_EVM_VERSION") {
            Some(v) => v.as_bytes(),
            None => include_bytes!("../../VERSION"),
        };
        io.return_output(version)
    }

    /// Get owner account id for this contract.
    #[no_mangle]
    pub extern "C" fn get_owner() {
        let mut io = Runtime;
        let state = engine::get_state(&io).sdk_unwrap();
        io.return_output(state.owner_id.as_bytes());
    }

    /// Get bridge prover id for this contract.
    #[no_mangle]
    pub extern "C" fn get_bridge_prover() {
        let mut io = Runtime;
        let connector = EthConnectorContract::init_instance(io).sdk_unwrap();
        io.return_output(connector.get_bridge_prover().as_bytes());
    }

    /// Get chain id for this contract.
    #[no_mangle]
    pub extern "C" fn get_chain_id() {
        let mut io = Runtime;
        io.return_output(&engine::get_state(&io).sdk_unwrap().chain_id)
    }

    #[no_mangle]
    pub extern "C" fn get_upgrade_index() {
        let mut io = Runtime;
        let state = engine::get_state(&io).sdk_unwrap();
        let index = internal_get_upgrade_index();
        io.return_output(&(index + state.upgrade_delay_blocks).to_le_bytes())
    }

    /// Stage new code for deployment.
    #[no_mangle]
    pub extern "C" fn stage_upgrade() {
        let mut io = Runtime;
        let state = engine::get_state(&io).sdk_unwrap();
        let block_height = io.block_height();
        require_owner_only(&state, &io.predecessor_account_id());
        io.read_input_and_store(&bytes_to_key(KeyPrefix::Config, CODE_KEY));
        io.write_storage(
            &bytes_to_key(KeyPrefix::Config, CODE_STAGE_KEY),
            &block_height.to_le_bytes(),
        );
    }

    /// Deploy staged upgrade.
    #[no_mangle]
    pub extern "C" fn deploy_upgrade() {
        let io = Runtime;
        let state = engine::get_state(&io).sdk_unwrap();
        require_owner_only(&state, &io.predecessor_account_id());
        let index = internal_get_upgrade_index();
        if io.block_height() <= index + state.upgrade_delay_blocks {
            sdk::panic_utf8(errors::ERR_NOT_ALLOWED_TOO_EARLY);
        }
        Runtime::self_deploy(&bytes_to_key(KeyPrefix::Config, CODE_KEY));
    }

    /// Called as part of the upgrade process (see `engine-sdk::self_deploy`). This function is meant
    /// to make any necessary changes to the state such that it aligns with the newly deployed
    /// code.
    #[no_mangle]
    pub extern "C" fn state_migration() {
        // TODO: currently we don't have migrations
    }

    /// Resumes previously [`paused`] precompiles.
    ///
    /// [`paused`]: crate::contract::pause_precompiles
    #[no_mangle]
    pub extern "C" fn resume_precompiles() {
        let io = Runtime;
        let state = engine::get_state(&io).sdk_unwrap();
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
        let authorizer: pausables::EngineAuthorizer = engine::get_authorizer();

        if !authorizer.is_authorized(&io.predecessor_account_id()) {
            sdk::panic_utf8("ERR_UNAUTHORIZED".as_bytes());
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
        io.return_output(&data[..]);
    }

    ///
    /// MUTATIVE METHODS
    ///

    /// Deploy code into the EVM.
    #[no_mangle]
    pub extern "C" fn deploy_code() {
        let io = Runtime;
        let input = io.read_input().to_vec();
        let current_account_id = io.current_account_id();
        let mut engine = Engine::new(
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
        let bytes = io.read_input().to_vec();
        let args = CallArgs::deserialize(&bytes).sdk_expect(errors::ERR_BORSH_DESERIALIZE);
        let current_account_id = io.current_account_id();
        let mut engine = Engine::new(
            predecessor_address(&io.predecessor_account_id()),
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
    /// Must match CHAIN_ID to make sure it's signed for given chain vs replayed from another chain.
    #[no_mangle]
    pub extern "C" fn submit() {
        let io = Runtime;
        let input = io.read_input().to_vec();
        let current_account_id = io.current_account_id();
        let state = engine::get_state(&io).sdk_unwrap();
        let relayer_address = predecessor_address(&io.predecessor_account_id());
        let result = engine::submit(
            io,
            &io,
            &input,
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
        let relayer_address = io.read_input_arr20().sdk_unwrap();

        let current_account_id = io.current_account_id();
        let predecessor_account_id = io.predecessor_account_id();
        let mut engine = Engine::new(
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
        let state = engine::get_state(&io).sdk_unwrap();
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
        io.assert_private_call().sdk_unwrap();
        let check_deploy: Result<(), &[u8]> = match io.promise_result(0) {
            Some(PromiseResult::Successful(_)) => Ok(()),
            Some(_) => Err(b"ERR_ROUTER_DEPLOY_FAILED"),
            None => Err(b"ERR_ROUTER_UPDATE_NOT_CALLBACK"),
        };
        check_deploy.sdk_unwrap();
        let args: crate::xcc::AddressVersionUpdateArgs = io.read_input_borsh().sdk_unwrap();
        crate::xcc::set_code_version_of_address(&mut io, &args.address, args.version);
    }

    /// Sets the address for the wNEAR ERC-20 contract. This contract will be used by the
    /// cross-contract calls feature to have users pay for their NEAR transactions.
    #[no_mangle]
    pub extern "C" fn factory_set_wnear_address() {
        let mut io = Runtime;
        let state = engine::get_state(&io).sdk_unwrap();
        require_owner_only(&state, &io.predecessor_account_id());
        let address = io.read_input_arr20().sdk_unwrap();
        crate::xcc::set_wnear_address(&mut io, &Address::from_array(address));
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
        let mut engine = Engine::new(
            predecessor_address(&predecessor_account_id),
            current_account_id.clone(),
            io,
            &io,
        )
        .sdk_unwrap();

        let args: NEP141FtOnTransferArgs = parse_json(io.read_input().to_vec().as_slice())
            .sdk_unwrap()
            .try_into()
            .sdk_unwrap();

        if predecessor_account_id == current_account_id {
            EthConnectorContract::init_instance(io)
                .sdk_unwrap()
                .ft_on_transfer(&engine, &args)
                .sdk_unwrap();
        } else {
            engine.receive_erc20_tokens(
                &predecessor_account_id,
                &args,
                &current_account_id,
                &mut Runtime,
            );
        }
    }

    /// Deploy ERC20 token mapped to a NEP141
    #[no_mangle]
    pub extern "C" fn deploy_erc20_token() {
        let mut io = Runtime;
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
            let state = engine::get_state(&io).sdk_unwrap();
            let refund_result =
                engine::refund_on_error(io, &io, state, args, &mut Runtime).sdk_unwrap();

            if !refund_result.status.is_ok() {
                sdk::panic_utf8(errors::ERR_REFUND_FAILURE);
            }
        }
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
        let engine = Engine::new(args.sender, current_account_id, io, &env).sdk_unwrap();
        let result = Engine::view_with_args(&engine, args).sdk_unwrap();
        io.return_output(&result.try_to_vec().sdk_expect(errors::ERR_SERIALIZE));
    }

    #[no_mangle]
    pub extern "C" fn get_block_hash() {
        let mut io = Runtime;
        let block_height = io.read_input_borsh().sdk_unwrap();
        let account_id = io.current_account_id();
        let chain_id = engine::get_state(&io)
            .map(|state| state.chain_id)
            .sdk_unwrap();
        let block_hash =
            crate::engine::compute_block_hash(chain_id, block_height, account_id.as_bytes());
        io.return_output(block_hash.as_bytes())
    }

    #[no_mangle]
    pub extern "C" fn get_code() {
        let mut io = Runtime;
        let address = io.read_input_arr20().sdk_unwrap();
        let code = engine::get_code(&io, &Address::from_array(address));
        io.return_output(&code)
    }

    #[no_mangle]
    pub extern "C" fn get_balance() {
        let mut io = Runtime;
        let address = io.read_input_arr20().sdk_unwrap();
        let balance = engine::get_balance(&io, &Address::from_array(address));
        io.return_output(&balance.to_bytes())
    }

    #[no_mangle]
    pub extern "C" fn get_nonce() {
        let mut io = Runtime;
        let address = io.read_input_arr20().sdk_unwrap();
        let nonce = engine::get_nonce(&io, &Address::from_array(address));
        io.return_output(&u256_to_arr(&nonce))
    }

    #[no_mangle]
    pub extern "C" fn get_storage_at() {
        let mut io = Runtime;
        let args: GetStorageAtArgs = io.read_input_borsh().sdk_unwrap();
        let address = args.address;
        let generation = engine::get_generation(&io, &address);
        let value = engine::get_storage(&io, &args.address, &H256(args.key), generation);
        io.return_output(&value.0)
    }

    ///
    /// BENCHMARKING METHODS
    ///
    #[cfg(feature = "evm_bully")]
    #[no_mangle]
    pub extern "C" fn begin_chain() {
        use crate::prelude::U256;
        let mut io = Runtime;
        let mut state = engine::get_state(&io).sdk_unwrap();
        require_owner_only(&state, &io.predecessor_account_id());
        let args: BeginChainArgs = io.read_input_borsh().sdk_unwrap();
        state.chain_id = args.chain_id;
        engine::set_state(&mut io, state);
        // set genesis block balances
        for account_balance in args.genesis_alloc {
            engine::set_balance(
                &mut io,
                &account_balance.address,
                &crate::prelude::Wei::new(U256::from(account_balance.balance)),
            )
        }
        // return new chain ID
        io.return_output(&engine::get_state(&io).sdk_unwrap().chain_id)
    }

    #[cfg(feature = "evm_bully")]
    #[no_mangle]
    pub extern "C" fn begin_block() {
        let io = Runtime;
        let state = engine::get_state(&io).sdk_unwrap();
        require_owner_only(&state, &io.predecessor_account_id());
        let _args: BeginBlockArgs = io.read_input_borsh().sdk_unwrap();
        // TODO: https://github.com/aurora-is-near/aurora-engine/issues/2
    }

    #[no_mangle]
    pub extern "C" fn new_eth_connector() {
        let io = Runtime;
        // Only the owner can initialize the EthConnector
        io.assert_private_call().sdk_unwrap();

        let args: InitCallArgs = io.read_input_borsh().sdk_unwrap();
        let owner_id = io.current_account_id();

        EthConnectorContract::create_contract(io, owner_id, args).sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn set_eth_connector_contract_data() {
        let mut io = Runtime;
        // Only the owner can set the EthConnector contract data
        io.assert_private_call().sdk_unwrap();

        let args: SetContractDataCallArgs = io.read_input_borsh().sdk_unwrap();
        connector::set_contract_data(&mut io, args).sdk_unwrap();
    }

    #[no_mangle]

    pub extern "C" fn withdraw() {
        let io = Runtime;
        io.assert_one_yocto().sdk_unwrap();
        let args = io.read_input_borsh().sdk_unwrap();
        let current_account_id = io.current_account_id();
        let predecessor_account_id = io.predecessor_account_id();
        let result = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .withdraw_eth_from_near(&current_account_id, &predecessor_account_id, args)
            .sdk_unwrap();
        let result_bytes = result.try_to_vec().sdk_expect(errors::ERR_SERIALIZE);
        // We intentionally do not go through the `io` struct here because we must bypass
        // the check that prevents output that is accepted by the eth_custodian
        #[allow(clippy::as_conversions)]
        unsafe {
            exports::value_return(
                u64::try_from(result_bytes.len()).sdk_expect(errors::ERR_VALUE_CONVERSION),
                result_bytes.as_ptr() as u64,
            );
        }
    }

    #[no_mangle]
    pub extern "C" fn deposit() {
        let mut io = Runtime;
        let raw_proof = io.read_input().to_vec();
        let current_account_id = io.current_account_id();
        let predecessor_account_id = io.predecessor_account_id();
        let promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .deposit(raw_proof, current_account_id, predecessor_account_id)
            .sdk_unwrap();
        // Safety: this call is safe because it comes from the eth-connector, not users.
        // The call is to verify the user-supplied proof for the deposit, with `finish_deposit`
        // as a callback.
        let promise_id = unsafe { io.promise_create_with_callback(&promise_args) };
        io.promise_return(promise_id);
    }

    #[no_mangle]
    pub extern "C" fn finish_deposit() {
        let mut io = Runtime;
        io.assert_private_call().sdk_unwrap();

        // Check result from proof verification call
        if io.promise_results_count() != 1 {
            sdk::panic_utf8(errors::ERR_PROMISE_COUNT);
        }
        let promise_result = match io.promise_result(0) {
            Some(PromiseResult::Successful(bytes)) => {
                bool::try_from_slice(&bytes).sdk_expect(errors::ERR_PROMISE_ENCODING)
            }
            _ => sdk::panic_utf8(errors::ERR_PROMISE_FAILED),
        };
        if !promise_result {
            sdk::panic_utf8(errors::ERR_VERIFY_PROOF);
        }

        let data = io.read_input_borsh().sdk_unwrap();
        let current_account_id = io.current_account_id();
        let predecessor_account_id = io.predecessor_account_id();
        let maybe_promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .finish_deposit(
                predecessor_account_id,
                current_account_id,
                data,
                io.prepaid_gas(),
            )
            .sdk_unwrap();

        if let Some(promise_args) = maybe_promise_args {
            // Safety: this call is safe because it comes from the eth-connector, not users.
            // The call will be to the Engine's ft_transfer_call`, which is needed as part
            // of the bridge flow (if depositing ETH to an Aurora address).
            let promise_id = unsafe { io.promise_create_with_callback(&promise_args) };
            io.promise_return(promise_id);
        }
    }

    #[no_mangle]
    pub extern "C" fn is_used_proof() {
        let mut io = Runtime;
        let args: IsUsedProofCallArgs = io.read_input_borsh().sdk_unwrap();

        let is_used_proof = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .is_used_proof(args.proof);
        let res = is_used_proof.try_to_vec().unwrap();
        io.return_output(&res[..]);
    }

    #[no_mangle]
    pub extern "C" fn ft_total_supply() {
        let io = Runtime;
        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .ft_total_eth_supply_on_near();
    }

    #[no_mangle]
    pub extern "C" fn ft_total_eth_supply_on_near() {
        let io = Runtime;
        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .ft_total_eth_supply_on_near();
    }

    #[no_mangle]
    pub extern "C" fn ft_total_eth_supply_on_aurora() {
        let io = Runtime;
        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .ft_total_eth_supply_on_aurora();
    }

    #[no_mangle]
    pub extern "C" fn ft_balance_of() {
        let io = Runtime;
        let args = parameters::BalanceOfCallArgs::try_from(
            parse_json(&io.read_input().to_vec()).sdk_unwrap(),
        )
        .sdk_unwrap();
        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .ft_balance_of(args);
    }

    #[no_mangle]
    pub extern "C" fn ft_balance_of_eth() {
        let io = Runtime;
        let args: parameters::BalanceOfEthCallArgs = io.read_input().to_value().sdk_unwrap();
        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .ft_balance_of_eth_on_aurora(args)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn ft_transfer() {
        let io = Runtime;
        io.assert_one_yocto().sdk_unwrap();
        let predecessor_account_id = io.predecessor_account_id();
        let args = parameters::TransferCallArgs::try_from(
            parse_json(&io.read_input().to_vec()).sdk_unwrap(),
        )
        .sdk_unwrap();
        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .ft_transfer(&predecessor_account_id, args)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn ft_resolve_transfer() {
        let io = Runtime;

        io.assert_private_call().sdk_unwrap();
        if io.promise_results_count() != 1 {
            sdk::panic_utf8(errors::ERR_PROMISE_COUNT);
        }

        let args: ResolveTransferCallArgs = io.read_input().to_value().sdk_unwrap();
        let promise_result = io.promise_result(0).sdk_unwrap();

        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .ft_resolve_transfer(args, promise_result);
    }

    #[no_mangle]
    pub extern "C" fn ft_transfer_call() {
        use sdk::types::ExpectUtf8;
        let mut io = Runtime;
        // Check is payable
        io.assert_one_yocto().sdk_unwrap();

        let args = TransferCallCallArgs::try_from(
            parse_json(&io.read_input().to_vec()).expect_utf8(ERR_FAILED_PARSE.as_bytes()),
        )
        .sdk_unwrap();
        let current_account_id = io.current_account_id();
        let predecessor_account_id = io.predecessor_account_id();
        let promise_args = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .ft_transfer_call(
                predecessor_account_id,
                current_account_id,
                args,
                io.prepaid_gas(),
            )
            .sdk_unwrap();
        // Safety: this call is safe. It is required by the NEP-141 spec that `ft_transfer_call`
        // creates a call to another contract's `ft_on_transfer` method.
        let promise_id = unsafe { io.promise_create_with_callback(&promise_args) };
        io.promise_return(promise_id);
    }

    #[no_mangle]
    pub extern "C" fn storage_deposit() {
        let mut io = Runtime;
        let args = StorageDepositCallArgs::from(parse_json(&io.read_input().to_vec()).sdk_unwrap());
        let predecessor_account_id = io.predecessor_account_id();
        let amount = Yocto::new(io.attached_deposit());
        let maybe_promise = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .storage_deposit(predecessor_account_id, amount, args)
            .sdk_unwrap();
        if let Some(promise) = maybe_promise {
            // Safety: This call is safe. It is only a transfer back to the user in the case
            // that they over paid for their deposit.
            unsafe { io.promise_create_batch(&promise) };
        }
    }

    #[no_mangle]
    pub extern "C" fn storage_unregister() {
        let mut io = Runtime;
        io.assert_one_yocto().sdk_unwrap();
        let predecessor_account_id = io.predecessor_account_id();
        let force = parse_json(&io.read_input().to_vec()).and_then(|args| args.bool("force").ok());
        let maybe_promise = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .storage_unregister(predecessor_account_id, force)
            .sdk_unwrap();
        if let Some(promise) = maybe_promise {
            // Safety: This call is safe. It is only a transfer back to the user for their deposit.
            unsafe { io.promise_create_batch(&promise) };
        }
    }

    #[no_mangle]
    pub extern "C" fn storage_withdraw() {
        let io = Runtime;
        io.assert_one_yocto().sdk_unwrap();
        let args =
            StorageWithdrawCallArgs::from(parse_json(&io.read_input().to_vec()).sdk_unwrap());
        let predecessor_account_id = io.predecessor_account_id();
        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .storage_withdraw(&predecessor_account_id, args)
            .sdk_unwrap()
    }

    #[no_mangle]
    pub extern "C" fn storage_balance_of() {
        let io = Runtime;
        let args = parameters::StorageBalanceOfCallArgs::try_from(
            parse_json(&io.read_input().to_vec()).sdk_unwrap(),
        )
        .sdk_unwrap();
        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .storage_balance_of(args)
    }

    #[no_mangle]
    pub extern "C" fn get_paused_flags() {
        let mut io = Runtime;
        let paused_flags = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .get_paused_flags();
        let data = paused_flags.try_to_vec().expect(ERR_FAILED_PARSE);
        io.return_output(&data[..]);
    }

    #[no_mangle]
    pub extern "C" fn set_paused_flags() {
        let io = Runtime;
        io.assert_private_call().sdk_unwrap();

        let args: PauseEthConnectorCallArgs = io.read_input_borsh().sdk_unwrap();
        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .set_paused_flags(args);
    }

    #[no_mangle]
    pub extern "C" fn get_accounts_counter() {
        let io = Runtime;
        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .get_accounts_counter();
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

    #[no_mangle]
    pub extern "C" fn ft_metadata() {
        let mut io = Runtime;
        let metadata: FungibleTokenMetadata = connector::get_metadata(&io).unwrap_or_default();
        let json_data = crate::json::JsonValue::from(metadata);
        io.return_output(json_data.to_string().as_bytes())
    }

    #[cfg(feature = "integration-test")]
    #[no_mangle]
    pub extern "C" fn verify_log_entry() {
        sdk::log!("Call from verify_log_entry");
        let mut io = Runtime;
        let data = true.try_to_vec().unwrap();
        io.return_output(&data[..]);
    }

    /// Function used to create accounts for tests
    #[cfg(feature = "integration-test")]
    #[no_mangle]
    pub extern "C" fn mint_account() {
        use crate::connector::ZERO_ATTACHED_BALANCE;
        use crate::prelude::{NEP141Wei, U256};
        use evm::backend::ApplyBackend;
        const GAS_FOR_VERIFY: NearGas = NearGas::new(20_000_000_000_000);
        const GAS_FOR_FINISH: NearGas = NearGas::new(50_000_000_000_000);

        let mut io = Runtime;
        let args: ([u8; 20], u64, u64) = io.read_input_borsh().sdk_expect(errors::ERR_ARGS);
        let address = Address::from_array(args.0);
        let nonce = U256::from(args.1);
        let balance = NEP141Wei::new(u128::from(args.2));
        let current_account_id = io.current_account_id();
        let mut engine = Engine::new(address, current_account_id, io, &io).sdk_unwrap();
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

        // Call "finish_deposit" to mint the corresponding
        // nETH NEP-141 tokens as well
        let aurora_account_id = io.current_account_id();
        let args = crate::parameters::FinishDepositCallArgs {
            new_owner_id: aurora_account_id.clone(),
            amount: balance,
            proof_key: crate::prelude::String::new(),
            relayer_id: aurora_account_id.clone(),
            fee: 0.into(),
            msg: None,
        };
        let verify_call = aurora_engine_types::parameters::PromiseCreateArgs {
            target_account_id: aurora_account_id.clone(),
            method: "verify_log_entry".to_string(),
            args: crate::prelude::Vec::new(),
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: GAS_FOR_VERIFY,
        };
        let finish_call = aurora_engine_types::parameters::PromiseCreateArgs {
            target_account_id: aurora_account_id,
            method: "finish_deposit".to_string(),
            args: args.try_to_vec().unwrap(),
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: GAS_FOR_FINISH,
        };
        // Safety: this call is safe because it is only used in integration tests.
        unsafe {
            io.promise_create_with_callback(
                &aurora_engine_types::parameters::PromiseWithCallbackArgs {
                    base: verify_call,
                    callback: finish_call,
                },
            )
        };
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

    fn require_owner_only(state: &EngineState, predecessor_account_id: &AccountId) {
        if &state.owner_id != predecessor_account_id {
            sdk::panic_utf8(errors::ERR_NOT_ALLOWED);
        }
    }

    fn predecessor_address(predecessor_account_id: &AccountId) -> Address {
        near_account_to_evm_address(predecessor_account_id.as_bytes())
    }

    mod exports {
        extern "C" {
            pub(crate) fn value_return(value_len: u64, value_ptr: u64);
        }
    }
}

pub trait AuroraState {
    fn add_promise(&mut self, promise: PromiseCreateArgs);
}

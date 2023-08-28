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

use aurora_engine_types::parameters::PromiseCreateArgs;

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
pub mod contract_methods;
pub mod deposit_event;
pub mod engine;
pub mod errors;
pub mod fungible_token;
pub mod pausables;
mod prelude;
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
    use crate::connector::{self, EthConnectorContract};
    use crate::engine::{self, Engine};
    use crate::parameters::{
        self, FungibleTokenMetadata, GetErc20FromNep141CallArgs, GetStorageAtArgs,
        IsUsedProofCallArgs, ViewCallArgs,
    };
    #[cfg(feature = "evm_bully")]
    use crate::parameters::{BeginBlockArgs, BeginChainArgs};
    use crate::prelude::sdk::types::{SdkExpect, SdkUnwrap};
    use crate::prelude::storage::{bytes_to_key, KeyPrefix};
    use crate::prelude::{sdk, u256_to_arr, Address, ERR_FAILED_PARSE, H256};
    use crate::{
        contract_methods::{self, ContractError},
        errors, state,
    };
    use aurora_engine_sdk::env::Env;
    use aurora_engine_sdk::io::{StorageIntermediate, IO};
    use aurora_engine_sdk::near_runtime::{Runtime, ViewEnv};
    use aurora_engine_types::borsh::BorshSerialize;
    use aurora_engine_types::parameters::engine::errors::ParseTypeFromJsonError;

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
        let io = Runtime;
        contract_methods::admin::new(io)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Get version of the contract.
    #[no_mangle]
    pub extern "C" fn get_version() {
        let io = Runtime;
        contract_methods::admin::get_version(io)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Get owner account id for this contract.
    #[no_mangle]
    pub extern "C" fn get_owner() {
        let io = Runtime;
        contract_methods::admin::get_owner(io)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Set owner account id for this contract.
    #[no_mangle]
    pub extern "C" fn set_owner() {
        let io = Runtime;
        let env = Runtime;
        contract_methods::admin::set_owner(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Get bridge prover id for this contract.
    #[no_mangle]
    pub extern "C" fn get_bridge_prover() {
        let io = Runtime;
        contract_methods::admin::get_bridge_prover(io)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Get chain id for this contract.
    #[no_mangle]
    pub extern "C" fn get_chain_id() {
        let io = Runtime;
        contract_methods::admin::get_chain_id(io)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn get_upgrade_delay_blocks() {
        let io = Runtime;
        contract_methods::admin::get_upgrade_delay_blocks(io)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn set_upgrade_delay_blocks() {
        let io = Runtime;
        let env = Runtime;
        contract_methods::admin::set_upgrade_delay_blocks(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn get_upgrade_index() {
        let io = Runtime;
        contract_methods::admin::get_upgrade_index(io)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Stage new code for deployment.
    #[no_mangle]
    pub extern "C" fn stage_upgrade() {
        let io = Runtime;
        let env = Runtime;
        contract_methods::admin::stage_upgrade(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Deploy staged upgrade.
    #[no_mangle]
    pub extern "C" fn deploy_upgrade() {
        // This function is intentionally not implemented in `contract_methods`
        // because it only make sense in the context of the Near runtime.
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
        let env = Runtime;
        contract_methods::admin::resume_precompiles(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Pauses a precompile.
    #[no_mangle]
    pub extern "C" fn pause_precompiles() {
        let io = Runtime;
        let env = Runtime;
        contract_methods::admin::pause_precompiles(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Returns an unsigned integer where each 1-bit means that a precompile corresponding to that bit is paused and
    /// 0-bit means not paused.
    #[no_mangle]
    pub extern "C" fn paused_precompiles() {
        let io = Runtime;
        contract_methods::admin::paused_precompiles(io)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Sets the flag to pause the contract.
    #[no_mangle]
    pub extern "C" fn pause_contract() {
        let io = Runtime;
        let env = Runtime;
        contract_methods::admin::pause_contract(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Sets the flag to resume the contract.
    #[no_mangle]
    pub extern "C" fn resume_contract() {
        let io = Runtime;
        let env = Runtime;
        contract_methods::admin::resume_contract(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    ///
    /// MUTATIVE METHODS
    ///

    /// Deploy code into the EVM.
    #[no_mangle]
    pub extern "C" fn deploy_code() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        contract_methods::evm_transactions::deploy_code(io, &env, &mut handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Call method on the EVM contract.
    #[no_mangle]
    pub extern "C" fn call() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        contract_methods::evm_transactions::call(io, &env, &mut handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Process signed Ethereum transaction.
    /// Must match `CHAIN_ID` to make sure it's signed for given chain vs replayed from another chain.
    #[no_mangle]
    pub extern "C" fn submit() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        contract_methods::evm_transactions::submit(io, &env, &mut handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Analog of the `submit` function, but waits for the `SubmitArgs` structure rather than
    /// the array of bytes representing the transaction.
    #[no_mangle]
    pub extern "C" fn submit_with_args() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        contract_methods::evm_transactions::submit_with_args(io, &env, &mut handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn register_relayer() {
        let io = Runtime;
        let env = Runtime;
        contract_methods::admin::register_relayer(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Updates the bytecode for user's router contracts created by the engine.
    /// These contracts are where cross-contract calls initiated by the EVM precompile
    /// will be sent from.
    #[no_mangle]
    pub extern "C" fn factory_update() {
        let io = Runtime;
        let env = Runtime;
        contract_methods::xcc::factory_update(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Updates the bytecode version for the given account. This is only called as a callback
    /// when a new version of the router contract is deployed to an account.
    #[no_mangle]
    pub extern "C" fn factory_update_address_version() {
        let io = Runtime;
        let env = Runtime;
        let handler = Runtime;
        contract_methods::xcc::factory_update_address_version(io, &env, &handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Sets the address for the `wNEAR` ERC-20 contract. This contract will be used by the
    /// cross-contract calls feature to have users pay for their NEAR transactions.
    #[no_mangle]
    pub extern "C" fn factory_set_wnear_address() {
        let io = Runtime;
        let env = Runtime;
        contract_methods::xcc::factory_set_wnear_address(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Returns the address for the `wNEAR` ERC-20 contract in borsh format.
    #[no_mangle]
    pub extern "C" fn factory_get_wnear_address() {
        let io = Runtime;
        contract_methods::xcc::factory_get_wnear_address(io)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Create and/or fund an XCC sub-account directly (as opposed to having one be automatically
    /// created via the XCC precompile in the EVM). The purpose of this method is to enable
    /// XCC on engine instances where wrapped NEAR (WNEAR) is not bridged.
    #[no_mangle]
    pub extern "C" fn fund_xcc_sub_account() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        contract_methods::xcc::fund_xcc_sub_account(&io, &env, &mut handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Allow receiving NEP141 tokens to the EVM contract.
    ///
    /// This function returns the amount of tokens to return to the sender.
    /// Either all tokens are transferred and tokens are returned
    /// in case of an error, or no token is returned if the transaction was successful.
    #[no_mangle]
    pub extern "C" fn ft_on_transfer() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        contract_methods::connector::ft_on_transfer(io, &env, &mut handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Deploy ERC20 token mapped to a NEP141
    #[no_mangle]
    pub extern "C" fn deploy_erc20_token() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        contract_methods::connector::deploy_erc20_token(io, &env, &mut handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Callback invoked by exit to NEAR precompile to handle potential
    /// errors in the exit call.
    #[no_mangle]
    pub extern "C" fn refund_on_error() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        contract_methods::connector::refund_on_error(io, &env, &mut handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Sets relayer key manager.
    #[no_mangle]
    pub extern "C" fn set_key_manager() {
        let io = Runtime;
        let env = Runtime;
        contract_methods::admin::set_key_manager(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Adds a relayer function call key.
    #[no_mangle]
    pub extern "C" fn add_relayer_key() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        contract_methods::admin::add_relayer_key(io, &env, &mut handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    /// Removes a relayer function call key.
    #[no_mangle]
    pub extern "C" fn remove_relayer_key() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        contract_methods::admin::remove_relayer_key(io, &env, &mut handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
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
        let block_hash =
            crate::engine::compute_block_hash(chain_id, block_height, account_id.as_bytes());
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

    #[no_mangle]
    pub extern "C" fn new_eth_connector() {
        let io = Runtime;
        let env = Runtime;
        contract_methods::connector::new_eth_connector(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn set_eth_connector_contract_data() {
        let io = Runtime;
        let env = Runtime;
        contract_methods::connector::set_eth_connector_contract_data(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn withdraw() {
        let io = Runtime;
        let env = Runtime;
        let result_bytes = contract_methods::connector::withdraw(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
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
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        let _ = contract_methods::connector::deposit(io, &env, &mut handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn finish_deposit() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        contract_methods::connector::finish_deposit(io, &env, &mut handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn is_used_proof() {
        let mut io = Runtime;
        let args: IsUsedProofCallArgs = io.read_input_borsh().sdk_unwrap();

        let is_used_proof = EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .is_used_proof(&args.proof);
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
        let args: parameters::BalanceOfCallArgs = serde_json::from_slice(&io.read_input().to_vec())
            .map_err(Into::<ParseTypeFromJsonError>::into)
            .sdk_unwrap();
        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .ft_balance_of(&args);
    }

    #[no_mangle]
    pub extern "C" fn ft_balance_of_eth() {
        let io = Runtime;
        let args: parameters::BalanceOfEthCallArgs = io.read_input().to_value().sdk_unwrap();
        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .ft_balance_of_eth_on_aurora(&args)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn ft_transfer() {
        let io = Runtime;
        let env = Runtime;
        contract_methods::connector::ft_transfer(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn ft_resolve_transfer() {
        let io = Runtime;
        let env = Runtime;
        let handler = Runtime;
        contract_methods::connector::ft_resolve_transfer(io, &env, &handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn ft_transfer_call() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        let _ = contract_methods::connector::ft_transfer_call(io, &env, &mut handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn storage_deposit() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        contract_methods::connector::storage_deposit(io, &env, &mut handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn storage_unregister() {
        let io = Runtime;
        let env = Runtime;
        let mut handler = Runtime;
        contract_methods::connector::storage_unregister(io, &env, &mut handler)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn storage_withdraw() {
        let io = Runtime;
        let env = Runtime;
        contract_methods::connector::storage_withdraw(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
    }

    #[no_mangle]
    pub extern "C" fn storage_balance_of() {
        let io = Runtime;
        let args: parameters::StorageBalanceOfCallArgs =
            serde_json::from_slice(&io.read_input().to_vec())
                .map_err(Into::<ParseTypeFromJsonError>::into)
                .sdk_unwrap();
        EthConnectorContract::init_instance(io)
            .sdk_unwrap()
            .storage_balance_of(&args);
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
        let env = Runtime;
        contract_methods::connector::set_paused_flags(io, &env)
            .map_err(ContractError::msg)
            .sdk_unwrap();
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
        let erc20_address: engine::ERC20Address = io.read_input().to_vec().try_into().sdk_unwrap();
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
        let bytes = serde_json::to_vec(&metadata).unwrap_or_default();
        io.return_output(&bytes);
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
            method: crate::prelude::String::from("verify_log_entry"),
            args: crate::prelude::Vec::new(),
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: GAS_FOR_VERIFY,
        };
        let finish_call = aurora_engine_types::parameters::PromiseCreateArgs {
            target_account_id: aurora_account_id,
            method: crate::prelude::String::from("finish_deposit"),
            args: args.try_to_vec().unwrap(),
            attached_balance: ZERO_ATTACHED_BALANCE,
            attached_gas: GAS_FOR_FINISH,
        };
        // Safety: this call is safe because it is only used in integration tests.
        unsafe {
            use aurora_engine_sdk::promise::PromiseHandler;
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

    fn require_running(state: &state::EngineState) {
        if state.is_paused {
            sdk::panic_utf8(errors::ERR_PAUSED);
        }
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

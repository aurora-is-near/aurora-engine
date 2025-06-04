use core::ffi;

use aurora_engine::{
    contract_methods::{self, silo, ContractError},
    engine::{self, Engine},
    errors,
    parameters::{GetStorageAtArgs, ViewCallArgs},
    state,
};
use aurora_engine_sdk::{
    env::Env,
    io::{StorageIntermediate, IO},
    near_runtime::{Runtime, ViewEnv},
};
use aurora_engine_types::{
    borsh,
    parameters::silo::FixedGasArgs,
    types::{u256_to_arr, Address},
    H256,
};

/// ADMINISTRATIVE METHODS
/// Sets the configuration for the Engine.
/// Should be called on deployment.
#[no_mangle]
pub extern "C" fn _native_new() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::new(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Get a version of the contract.
#[no_mangle]
pub extern "C" fn _native_get_version() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::admin::get_version(io);
    Box::into_raw(Box::new(result)).cast()
}

/// Get owner account id for this contract.
#[no_mangle]
pub extern "C" fn _native_get_owner() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::admin::get_owner(io);
    Box::into_raw(Box::new(result)).cast()
}

/// Set owner account id for this contract.
#[no_mangle]
pub extern "C" fn _native_set_owner() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::set_owner(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Get chain id for this contract.
#[no_mangle]
pub extern "C" fn _native_get_chain_id() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::admin::get_chain_id(io);
    Box::into_raw(Box::new(result)).cast()
}

#[no_mangle]
pub extern "C" fn _native_get_upgrade_delay_blocks() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::admin::get_upgrade_delay_blocks(io);
    Box::into_raw(Box::new(result)).cast()
}

#[no_mangle]
pub extern "C" fn _native_set_upgrade_delay_blocks() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::set_upgrade_delay_blocks(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

#[no_mangle]
pub extern "C" fn _native_get_upgrade_index() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::admin::get_upgrade_index(io);
    Box::into_raw(Box::new(result)).cast()
}

/// Upgrade the contract with the provided code bytes.
#[no_mangle]
pub extern "C" fn _native_upgrade() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;

    let result = contract_methods::admin::upgrade(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Stage new code for deployment.
#[no_mangle]
pub extern "C" fn _native_stage_upgrade() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::stage_upgrade(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Called as part of the upgrade process (see `engine-sdk::self_deploy`). This function is meant
/// to make any necessary changes to the state such that it aligns with the newly deployed
/// code.
#[no_mangle]
#[allow(clippy::missing_const_for_fn)]
pub extern "C" fn _native_state_migration() -> *mut ffi::c_void {
    // TODO: currently we don't have migrations
    core::ptr::null_mut()
}

/// Resumes previously [`paused`] precompiles.
///
/// [`paused`]: pause_precompiles
#[no_mangle]
pub extern "C" fn _native_resume_precompiles() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::resume_precompiles(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Pauses a precompile.
#[no_mangle]
pub extern "C" fn _native_pause_precompiles() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::pause_precompiles(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Returns an unsigned integer where each bit set to 1 means that corresponding precompile
/// to that bit is paused and 0-bit means not paused.
#[no_mangle]
pub extern "C" fn _native_paused_precompiles() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::admin::paused_precompiles(io);
    Box::into_raw(Box::new(result)).cast()
}

/// Sets the flag to pause the contract.
#[no_mangle]
pub extern "C" fn _native_pause_contract() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::pause_contract(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Sets the flag to resume the contract.
#[no_mangle]
pub extern "C" fn _native_resume_contract() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::resume_contract(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// MUTATIVE METHODS
/// Deploy code into the EVM.
#[no_mangle]
pub extern "C" fn _native_deploy_code() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::evm_transactions::deploy_code(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Call method on the EVM contract.
#[no_mangle]
pub extern "C" fn _native_call() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::evm_transactions::call(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Process signed Ethereum transaction.
/// Must match `CHAIN_ID` to make sure it's signed for given chain vs replayed from another chain.
#[no_mangle]
pub extern "C" fn _native_submit() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::evm_transactions::submit(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Analog of the `submit` function, but waits for the `SubmitArgs` structure rather than
/// the array of bytes representing the transaction.
#[no_mangle]
pub extern "C" fn _native_submit_with_args() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::evm_transactions::submit_with_args(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

#[no_mangle]
pub extern "C" fn _native_register_relayer() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::register_relayer(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Updates the bytecode for user's router contracts created by the engine.
/// These contracts are where cross-contract calls initiated by the EVM precompile
/// will be sent from.
#[no_mangle]
pub extern "C" fn _native_factory_update() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::xcc::factory_update(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Updates the bytecode version for the given account. This is only called as a callback
/// when a new version of the router contract is deployed to an account.
#[no_mangle]
pub extern "C" fn _native_factory_update_address_version() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let handler = Runtime;
    let result = contract_methods::xcc::factory_update_address_version(io, &env, &handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Sets the address for the `wNEAR` ERC-20 contract. This contract will be used by the
/// cross-contract calls feature to have users pay for their NEAR transactions.
#[no_mangle]
pub extern "C" fn _native_factory_set_wnear_address() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::xcc::factory_set_wnear_address(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Returns the address for the `wNEAR` ERC-20 contract in borsh format.
#[no_mangle]
pub extern "C" fn _native_factory_get_wnear_address() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::xcc::factory_get_wnear_address(io);
    Box::into_raw(Box::new(result)).cast()
}

/// Create and/or fund an XCC sub-account directly (as opposed to having one be automatically
/// created via the XCC precompile in the EVM). The purpose of this method is to enable
/// XCC on engine instances where wrapped NEAR (`wNEAR`) is not bridged.
#[no_mangle]
pub extern "C" fn _native_fund_xcc_sub_account() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::xcc::fund_xcc_sub_account(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// A private function (only callable by the contract itself) used as part of the XCC flow.
/// This function uses the exit to Near precompile to move wNear from Aurora to a user's
/// XCC account.
#[no_mangle]
pub extern "C" fn _native_withdraw_wnear_to_router() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::xcc::withdraw_wnear_to_router(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Mirror existing ERC-20 token on the main Aurora contract.
/// Notice: It works if the SILO mode is on.
#[no_mangle]
pub extern "C" fn _native_mirror_erc20_token() -> *mut ffi::c_void {
    let io = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::connector::mirror_erc20_token(io, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Callback used by the `mirror_erc20_token` function.
#[no_mangle]
pub extern "C" fn _native_mirror_erc20_token_callback() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::connector::mirror_erc20_token_callback(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Sets relayer key manager.
#[no_mangle]
pub extern "C" fn _native_set_key_manager() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::set_key_manager(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Adds a relayer function call key.
#[no_mangle]
pub extern "C" fn _native_add_relayer_key() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::admin::add_relayer_key(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Callback which is called by `add_relayer_key` and stores the relayer function
/// call key into the storage.
#[no_mangle]
pub extern "C" fn _native_store_relayer_key_callback() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::store_relayer_key_callback(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Removes a relayer function call key.
#[no_mangle]
pub extern "C" fn _native_remove_relayer_key() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::admin::remove_relayer_key(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Initialize the hashchain.
#[no_mangle]
pub extern "C" fn _native_start_hashchain() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::admin::start_hashchain(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

/// Attach a full access key.
#[no_mangle]
pub extern "C" fn _native_attach_full_access_key() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::admin::attach_full_access_key(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

///
/// READ-ONLY METHODS
///
#[no_mangle]
pub extern "C" fn _native_view() -> *mut ffi::c_void {
    // closure enables us to use `?` operator
    let f = move || -> Result<(), ContractError> {
        let mut io = Runtime;
        let env = ViewEnv;
        let args: ViewCallArgs = io.read_input_borsh()?;
        let current_account_id = io.current_account_id();
        let engine: Engine<_, _> = Engine::new(args.sender, current_account_id, io, &env)?;
        let result = Engine::view_with_args(&engine, args)?;
        io.return_output(&borsh::to_vec(&result).expect(errors::ERR_SERIALIZE));

        Ok(())
    };

    Box::into_raw(Box::new(f())).cast()
}

#[no_mangle]
pub extern "C" fn _native_get_block_hash() -> *mut ffi::c_void {
    let f = move || -> Result<(), ContractError> {
        let mut io = Runtime;
        let block_height = io.read_input_borsh()?;
        let account_id = io.current_account_id();
        let chain_id = state::get_state(&io).map(|state| state.chain_id)?;
        let block_hash = engine::compute_block_hash(chain_id, block_height, account_id.as_bytes());
        io.return_output(block_hash.as_bytes());

        Ok(())
    };

    Box::into_raw(Box::new(f())).cast()
}

#[no_mangle]
pub extern "C" fn _native_get_code() -> *mut ffi::c_void {
    let f = move || -> Result<(), ContractError> {
        let mut io = Runtime;
        let address = io.read_input_arr20()?;
        let code = engine::get_code(&io, &Address::from_array(address));
        io.return_output(&code);

        Ok(())
    };

    Box::into_raw(Box::new(f())).cast()
}

#[no_mangle]
pub extern "C" fn _native_get_balance() -> *mut ffi::c_void {
    let f = move || -> Result<(), ContractError> {
        let mut io = Runtime;
        let address = io.read_input_arr20()?;
        let balance = engine::get_balance(&io, &Address::from_array(address));
        io.return_output(&balance.to_bytes());

        Ok(())
    };

    Box::into_raw(Box::new(f())).cast()
}

#[no_mangle]
pub extern "C" fn _native_get_nonce() -> *mut ffi::c_void {
    let f = move || -> Result<(), ContractError> {
        let mut io = Runtime;
        let address = io.read_input_arr20()?;
        let nonce = engine::get_nonce(&io, &Address::from_array(address));
        io.return_output(&u256_to_arr(&nonce));

        Ok(())
    };

    Box::into_raw(Box::new(f())).cast()
}

#[no_mangle]
pub extern "C" fn _native_get_storage_at() -> *mut ffi::c_void {
    let f = move || -> Result<(), ContractError> {
        let mut io = Runtime;
        let args: GetStorageAtArgs = io.read_input_borsh()?;
        let address = args.address;
        let generation = engine::get_generation(&io, &address);
        let value = engine::get_storage(&io, &args.address, &H256(args.key), generation);
        io.return_output(&value.0);

        Ok(())
    };

    Box::into_raw(Box::new(f())).cast()
}

#[no_mangle]
pub extern "C" fn _native_get_latest_hashchain() -> *mut ffi::c_void {
    let mut io = Runtime;
    let result = contract_methods::admin::get_latest_hashchain(&mut io);
    Box::into_raw(Box::new(result)).cast()
}

/// Return metadata of the ERC-20 contract.
#[no_mangle]
pub extern "C" fn _native_get_erc20_metadata() -> *mut ffi::c_void {
    let io = Runtime;
    let env = ViewEnv;
    let result = contract_methods::connector::get_erc20_metadata(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

#[no_mangle]
pub extern "C" fn _native_withdraw() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::connector::withdraw(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

#[no_mangle]
pub extern "C" fn _native_ft_total_supply() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::connector::ft_total_eth_supply_on_near(io);
    Box::into_raw(Box::new(result)).cast()
}

#[no_mangle]
pub extern "C" fn _native_ft_total_eth_supply_on_near() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::connector::ft_total_eth_supply_on_near(io);
    Box::into_raw(Box::new(result)).cast()
}

#[no_mangle]
pub extern "C" fn _native_ft_balance_of() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::connector::ft_balance_of(io);
    Box::into_raw(Box::new(result)).cast()
}

#[no_mangle]
pub extern "C" fn _native_ft_balance_of_eth() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::connector::ft_balance_of_eth(io);
    Box::into_raw(Box::new(result)).cast()
}

/// Allows receiving NEP141 tokens in the EVM contract.
///
/// This function is called when NEP141 tokens are transferred to the contract.
/// It returns the amount of tokens that should be returned to the sender.
///
/// There are two possible outcomes:
/// 1. If an error occurs during the token transfer, all the transferred tokens are returned to the sender.
/// 2. If the token transfer is successful, no tokens are returned, and the contract keeps the transferred tokens.
#[no_mangle]
pub extern "C" fn _native_ft_on_transfer() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::connector::ft_on_transfer(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Deploy ERC20 token mapped to a NEP141
#[no_mangle]
pub extern "C" fn _native_deploy_erc20_token() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::connector::deploy_erc20_token(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Callback used by the `deploy_erc20_token` function.
#[no_mangle]
pub extern "C" fn _native_deploy_erc20_token_callback() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::connector::deploy_erc20_token_callback(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Set metadata of ERC-20 contract.
#[no_mangle]
pub extern "C" fn _native_set_erc20_metadata() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result = contract_methods::connector::set_erc20_metadata(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

/// Callback invoked by exit to NEAR precompile to handle potential
/// errors in the exit call or to perform the near tokens transfer.
#[no_mangle]
pub extern "C" fn _native_exit_to_near_precompile_callback() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let mut handler = Runtime;
    let result =
        contract_methods::connector::exit_to_near_precompile_callback(io, &env, &mut handler);
    Box::into_raw(Box::new(result)).cast()
}

#[no_mangle]
pub extern "C" fn _native_storage_balance_of() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::connector::storage_balance_of(io);
    Box::into_raw(Box::new(result)).cast()
}

#[no_mangle]
pub extern "C" fn _native_get_eth_connector_contract_account() -> *mut ffi::c_void {
    let io = Runtime;
    let result = contract_methods::connector::get_eth_connector_contract_account(io);
    Box::into_raw(Box::new(result)).cast()
}

#[no_mangle]
pub extern "C" fn _native_set_eth_connector_contract_account() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::connector::set_eth_connector_contract_account(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

#[no_mangle]
pub extern "C" fn _native_get_erc20_from_nep141() -> *mut ffi::c_void {
    let f = move || -> Result<(), ContractError> {
        let mut io = Runtime;
        let nep141 = io.read_input_borsh()?;
        let result = engine::get_erc20_from_nep141(&io, &nep141)?;
        io.return_output(result.as_bytes());

        Ok(())
    };
    Box::into_raw(Box::new(f())).cast()
}

#[no_mangle]
pub extern "C" fn _native_get_nep141_from_erc20() -> *mut ffi::c_void {
    let f = move || -> Result<(), ContractError> {
        let mut io = Runtime;
        let erc20_address: engine::ERC20Address = io.read_input().to_vec().try_into()?;
        let result = engine::nep141_erc20_map(io)
            .lookup_right(&erc20_address)
            .expect("ERC20_NOT_FOUND");
        io.return_output(result.as_ref());

        Ok(())
    };
    Box::into_raw(Box::new(f())).cast()
}

#[no_mangle]
pub extern "C" fn _native_ft_metadata() -> *mut ffi::c_void {
    let io = Runtime;
    let env = Runtime;
    let result = contract_methods::connector::ft_metadata(io, &env);
    Box::into_raw(Box::new(result)).cast()
}

///
/// Silo
///
#[no_mangle]
pub extern "C" fn _native_get_fixed_gas() -> *mut ffi::c_void {
    let f = move || -> Result<(), ContractError> {
        let mut io = Runtime;
        let args = FixedGasArgs {
            fixed_gas: silo::get_fixed_gas(&io),
        };

        let result = borsh::to_vec(&args).map_err(|e| e.to_string())?;
        io.return_output(result.as_ref());

        Ok(())
    };
    Box::into_raw(Box::new(f())).cast()
}

#[no_mangle]
pub extern "C" fn _native_silo_set_fixed_gas(args: *mut ffi::c_void) {
    let fixed_gas = *unsafe { Box::from_raw(args.cast()) };

    let mut io = Runtime;
    silo::set_fixed_gas(&mut io, fixed_gas);
}

#[no_mangle]
pub extern "C" fn _native_silo_set_erc20_fallback_address(args: *mut ffi::c_void) {
    let address = *unsafe { Box::from_raw(args.cast()) };

    let mut io = Runtime;
    silo::set_erc20_fallback_address(&mut io, address);
}

#[no_mangle]
pub extern "C" fn _native_silo_set_silo_params(args: *mut ffi::c_void) {
    let args = *unsafe { Box::from_raw(args.cast()) };

    let mut io = Runtime;
    silo::set_silo_params(&mut io, args);
}

#[no_mangle]
pub extern "C" fn _native_silo_add_entry_to_whitelist(args: *mut ffi::c_void) {
    let args = *unsafe { Box::from_raw(args.cast()) };

    let io = Runtime;
    silo::add_entry_to_whitelist(&io, args);
}

#[no_mangle]
pub extern "C" fn _native_silo_add_entry_to_whitelist_batch(args: *mut ffi::c_void) {
    let entries: Vec<_> = *unsafe { Box::from_raw(args.cast()) };

    let io = Runtime;
    silo::add_entry_to_whitelist_batch(&io, entries);
}

#[no_mangle]
pub extern "C" fn _native_silo_remove_entry_from_whitelist(args: *mut ffi::c_void) {
    let args = *unsafe { Box::from_raw(args.cast()) };

    let io = Runtime;
    silo::remove_entry_from_whitelist(&io, args);
}

#[no_mangle]
pub extern "C" fn _native_silo_set_whitelist_status(args: *mut ffi::c_void) {
    let args = *unsafe { Box::from_raw(args.cast()) };

    let io = Runtime;
    silo::set_whitelist_status(&io, args);
}

#[no_mangle]
pub extern "C" fn _native_silo_set_whitelists_statuses(args: *mut ffi::c_void) {
    let args: Vec<_> = *unsafe { Box::from_raw(args.cast()) };

    let io = Runtime;
    silo::set_whitelists_statuses(&io, args);
}

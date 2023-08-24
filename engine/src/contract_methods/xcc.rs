use crate::{
    contract_methods::{require_owner_only, require_running, ContractError},
    errors, state, xcc,
};
use aurora_engine_sdk::{
    env::Env,
    io::{StorageIntermediate, IO},
    promise::PromiseHandler,
};
use aurora_engine_types::{borsh::BorshSerialize, types::Address};

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

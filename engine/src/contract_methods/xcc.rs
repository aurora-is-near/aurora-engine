use crate::{
    contract_methods::{predecessor_address, require_owner_only, require_running, ContractError},
    engine::Engine,
    errors,
    hashchain::{with_hashchain, with_logs_hashchain},
    state, xcc,
};
use aurora_engine_modexp::AuroraModExp;
use aurora_engine_sdk::{
    env::Env,
    io::{StorageIntermediate, IO},
    promise::PromiseHandler,
};
use aurora_engine_types::{
    account_id::AccountId,
    borsh, format,
    parameters::{engine::SubmitResult, xcc::WithdrawWnearToRouterArgs},
    types::Address,
};
use function_name::named;

#[named]
pub fn withdraw_wnear_to_router<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<SubmitResult, ContractError> {
    with_logs_hashchain(io, env, function_name!(), |io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        env.assert_private_call()?;
        if matches!(handler.promise_result_check(), Some(false)) {
            return Err(b"ERR_CALLBACK_OF_FAILED_PROMISE".into());
        }
        let args: WithdrawWnearToRouterArgs = io.read_input_borsh()?;
        let current_account_id = env.current_account_id();
        let recipient = AccountId::new(&format!(
            "{}.{}",
            args.target.encode(),
            current_account_id.as_ref()
        ))?;
        let wnear_address = aurora_engine_precompiles::xcc::state::get_wnear_address(&io);
        let mut engine: Engine<_, E, AuroraModExp> = Engine::new_with_state(
            state,
            predecessor_address(&current_account_id),
            current_account_id,
            io,
            env,
        );
        let (result, ids) = xcc::withdraw_wnear_to_router(
            &recipient,
            args.amount,
            wnear_address,
            &mut engine,
            handler,
        )?;
        if !result.status.is_ok() {
            return Err(b"ERR_WITHDRAW_FAILED".into());
        }
        let id = ids.last().ok_or(b"ERR_NO_PROMISE_CREATED")?;
        handler.promise_return(*id);
        Ok(result)
    })
}

#[named]
pub fn factory_update<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        require_owner_only(&state, &env.predecessor_account_id())?;
        let bytes = io.read_input().to_vec();
        let router_bytecode = xcc::RouterCode::new(bytes);
        xcc::update_router_code(&mut io, &router_bytecode);
        Ok(())
    })
}

#[named]
pub fn factory_update_address_version<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &H,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
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
    })
}

#[named]
pub fn factory_set_wnear_address<I: IO + Copy, E: Env>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        require_owner_only(&state, &env.predecessor_account_id())?;
        let address = io.read_input_arr20()?;
        xcc::set_wnear_address(&mut io, &Address::from_array(address));
        Ok(())
    })
}

pub fn factory_get_wnear_address<I: IO + Copy>(mut io: I) -> Result<(), ContractError> {
    let address = aurora_engine_precompiles::xcc::state::get_wnear_address(&io);
    let bytes = borsh::to_vec(&address).map_err(|_| errors::ERR_SERIALIZE)?;
    io.return_output(&bytes);
    Ok(())
}

#[named]
pub fn fund_xcc_sub_account<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        // This method can only be called by the owner because it allows specifying the
        // account ID of the wNEAR account. This information must be accurate for the
        // sub-account to work properly, therefore this method can only be called by
        // a trusted user.
        require_owner_only(&state, &env.predecessor_account_id())?;
        let args: xcc::FundXccArgs = io.read_input_borsh()?;
        xcc::fund_xcc_sub_account(&io, handler, env, args)?;
        Ok(())
    })
}

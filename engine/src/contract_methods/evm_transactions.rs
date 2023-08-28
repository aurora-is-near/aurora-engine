use crate::{
    contract_methods::{predecessor_address, require_running, ContractError},
    engine::{self, Engine},
    errors, state,
};
use aurora_engine_modexp::AuroraModExp;
use aurora_engine_sdk::{
    env::Env,
    io::{StorageIntermediate, IO},
    promise::PromiseHandler,
};
use aurora_engine_types::{
    borsh::BorshSerialize,
    parameters::engine::{CallArgs, SubmitArgs, SubmitResult},
};

pub fn deploy_code<'env, I: IO + Copy, E: Env, H: PromiseHandler>(
    mut io: I,
    env: &'env E,
    handler: &mut H,
) -> Result<SubmitResult, ContractError> {
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
    Ok(result)
}

pub fn call<'env, I: IO + Copy, E: Env, H: PromiseHandler>(
    mut io: I,
    env: &'env E,
    handler: &mut H,
) -> Result<SubmitResult, ContractError> {
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
    Ok(result)
}

pub fn submit<I: IO + Copy, E: Env, H: PromiseHandler>(
    mut io: I,
    env: &E,
    handler: &mut H,
) -> Result<SubmitResult, ContractError> {
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

    Ok(result)
}

pub fn submit_with_args<I: IO + Copy, E: Env, H: PromiseHandler>(
    mut io: I,
    env: &E,
    handler: &mut H,
) -> Result<SubmitResult, ContractError> {
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

    Ok(result)
}

use crate::{
    connector::{self, EthConnectorContract},
    contract_methods::{predecessor_address, require_owner_only, require_running, ContractError},
    engine::{self, Engine},
    errors,
    hashchain::with_hashchain,
    state,
};
use aurora_engine_modexp::AuroraModExp;
use aurora_engine_sdk::{
    env::Env,
    io::{StorageIntermediate, IO},
    promise::PromiseHandler,
};
use aurora_engine_types::{
    borsh::{BorshDeserialize, BorshSerialize},
    parameters::{
        connector::{
            InitCallArgs, NEP141FtOnTransferArgs, ResolveTransferCallArgs, SetContractDataCallArgs,
            StorageDepositCallArgs, StorageWithdrawCallArgs, TransferCallArgs,
            TransferCallCallArgs,
        },
        engine::{
            errors::ParseTypeFromJsonError, DeployErc20TokenArgs, PauseEthConnectorCallArgs,
            SubmitResult,
        },
        PromiseWithCallbackArgs, RefundCallArgs,
    },
    types::{Address, PromiseResult, Yocto},
    Vec,
};
use function_name::named;

#[named]
pub fn ft_on_transfer<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        let current_account_id = env.current_account_id();
        let predecessor_account_id = env.predecessor_account_id();
        let mut engine: Engine<_, E, AuroraModExp> = Engine::new_with_state(
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
            engine.receive_erc20_tokens(
                &predecessor_account_id,
                &args,
                &current_account_id,
                handler,
            );
        }
        Ok(())
    })
}

#[named]
pub fn deploy_erc20_token<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<Address, ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
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
        Ok(address)
    })
}

#[named]
pub fn refund_on_error<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<Option<SubmitResult>, ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        env.assert_private_call()?;

        // This function should only be called as the callback of
        // exactly one promise.
        if handler.promise_results_count() != 1 {
            return Err(errors::ERR_PROMISE_COUNT.into());
        }

        let maybe_result = if let Some(PromiseResult::Successful(_)) = handler.promise_result(0) {
            // Promise succeeded -- nothing to do
            None
        } else {
            // Exit call failed; need to refund tokens
            let args: RefundCallArgs = io.read_input_borsh()?;
            let refund_result = engine::refund_on_error(io, env, state, &args, handler)?;

            if !refund_result.status.is_ok() {
                return Err(errors::ERR_REFUND_FAILURE.into());
            }
            Some(refund_result)
        };
        Ok(maybe_result)
    })
}

#[named]
pub fn new_eth_connector<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
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
    })
}

#[named]
pub fn set_eth_connector_contract_data<I: IO + Copy, E: Env>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
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
    })
}

#[named]
pub fn withdraw<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<Vec<u8>, ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
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
    })
}

#[named]
pub fn deposit<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<PromiseWithCallbackArgs, ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
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
        Ok(promise_args)
    })
}

#[named]
pub fn finish_deposit<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<Option<PromiseWithCallbackArgs>, ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
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

        if let Some(promise_args) = maybe_promise_args.as_ref() {
            // Safety: this call is safe because it comes from the eth-connector, not users.
            // The call will be to the Engine's ft_transfer_call`, which is needed as part
            // of the bridge flow (if depositing ETH to an Aurora address).
            let promise_id = unsafe { handler.promise_create_with_callback(promise_args) };
            handler.promise_return(promise_id);
        }

        Ok(maybe_promise_args)
    })
}

#[named]
pub fn ft_transfer<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        require_running(&state::get_state(&io)?)?;
        env.assert_one_yocto()?;
        let predecessor_account_id = env.predecessor_account_id();
        let args: TransferCallArgs = serde_json::from_slice(&io.read_input().to_vec())
            .map_err(Into::<ParseTypeFromJsonError>::into)?;
        EthConnectorContract::init_instance(io)?.ft_transfer(&predecessor_account_id, &args)?;
        Ok(())
    })
}

#[named]
pub fn ft_resolve_transfer<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &H,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
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
    })
}

#[named]
pub fn ft_transfer_call<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<PromiseWithCallbackArgs, ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
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
        Ok(promise_args)
    })
}

#[named]
pub fn storage_deposit<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
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
    })
}

#[named]
pub fn storage_unregister<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
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
    })
}

#[named]
pub fn storage_withdraw<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        require_running(&state::get_state(&io)?)?;
        env.assert_one_yocto()?;
        let args: StorageWithdrawCallArgs = serde_json::from_slice(&io.read_input().to_vec())
            .map_err(Into::<ParseTypeFromJsonError>::into)?;
        let predecessor_account_id = env.predecessor_account_id();
        EthConnectorContract::init_instance(io)?
            .storage_withdraw(&predecessor_account_id, &args)?;
        Ok(())
    })
}

#[named]
pub fn set_paused_flags<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        let is_private = env.assert_private_call();
        if is_private.is_err() {
            require_owner_only(&state, &env.predecessor_account_id())?;
        }
        let args: PauseEthConnectorCallArgs = io.read_input_borsh()?;
        EthConnectorContract::init_instance(io)?.set_paused_flags(&args);
        Ok(())
    })
}

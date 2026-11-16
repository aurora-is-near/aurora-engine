use aurora_engine_modexp::AuroraModExp;
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::error::BorshDeserializeError;
use aurora_engine_sdk::io::{IO, StorageIntermediate};
use aurora_engine_sdk::promise::PromiseHandler;
use aurora_engine_types::U256;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::borsh;
use aurora_engine_types::parameters::connector::{
    BridgedToken, EngineWithdrawCallArgs, Erc20Identifier, ExitToNearPrecompileCallbackArgs,
    FtOnTransferArgs, FtTransferArgs, FtTransferCallArgs, MirrorErc20TokenArgs, MtOnTransferArgs,
    Nep245Metadata, Nep245Token, RefundCallArgs, SetErc20MetadataArgs,
    SetEthConnectorContractAccountArgs, StorageDepositArgs, StorageUnregisterArgs,
    StorageWithdrawArgs, TransferCall, WithdrawCallArgs, WithdrawSerializeType,
};
use aurora_engine_types::parameters::engine::errors::ParseArgsError;
use aurora_engine_types::parameters::engine::{DeployErc20TokenArgs, SubmitResult};
use aurora_engine_types::parameters::{
    PromiseAction, PromiseBatchAction, PromiseCreateArgs, PromiseOrValue, PromiseWithCallbackArgs,
};
use aurora_engine_types::storage::{EthConnectorStorageId, KeyPrefix};
use aurora_engine_types::types::{Address, Balance, NearGas, PromiseResult, Yocto};
use function_name::named;

use crate::contract_methods::{
    ContractError, predecessor_address, require_owner_only, require_running,
};
use crate::engine::Engine;
use crate::hashchain::with_hashchain;
use crate::prelude::{ToString, Vec, sdk, vec};
use crate::{engine, errors, state};

#[cfg(test)]
mod tests;

const ONE_YOCTO: Yocto = Yocto::new(1);
/// Indicate zero attached balance for promise call
const ZERO_YOCTO: Yocto = Yocto::new(0);
/// Amount of attached gas for read-only promises.
const READ_PROMISE_ATTACHED_GAS: NearGas = NearGas::new(6_000_000_000_000);
/// Amount of attached gas for the `mirror_erc20_token_callback`.
const MIRROR_ERC20_TOKEN_CALLBACK_ATTACHED_GAS: NearGas = NearGas::new(10_000_000_000_000);
/// Amount of attached gas for the `deploy_erc20_token_callback`.
const DEPLOY_ERC20_TOKEN_CALLBACK_ATTACHED_GAS: NearGas = NearGas::new(30_000_000_000_000);
/// Amount of gas required for the promise creation.
const GAS_FOR_PROMISE_CREATION: NearGas = NearGas::new(2_000_000_000_000);
/// Maximum byte length of a NEP-141 and NEP-245 promise result accepted
/// by the [`exit_to_near_precompile_callback`].
const MAX_TRANSFER_RESULT_LENGTH: usize = 256;

pub fn withdraw<I: IO + Copy + PromiseHandler, E: Env>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    env.assert_one_yocto()?;

    let args: WithdrawCallArgs = io.read_input_borsh()?;
    let args = borsh::to_vec(&EngineWithdrawCallArgs {
        sender_id: env.predecessor_account_id(),
        recipient_address: args.recipient_address,
        amount: args.amount,
    })
    .unwrap();

    return_promise(io, env, "engine_withdraw", args, ONE_YOCTO)
}

#[named]
pub fn ft_on_transfer<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<Option<SubmitResult>, ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        let current_account_id = env.current_account_id();
        let predecessor_account_id = env.predecessor_account_id();
        let mut engine: Engine<_, _> = Engine::new_with_state(
            state,
            predecessor_address(&predecessor_account_id),
            current_account_id.clone(),
            io,
            env,
        );

        sdk::log!("Call ft_on_transfer");

        let args: FtOnTransferArgs = read_json_args(&io)?;
        let result = if predecessor_account_id == get_connector_account_id(&io)? {
            engine.receive_base_tokens(&args)
        } else {
            engine.receive_erc20_tokens(
                &BridgedToken::Nep141(predecessor_account_id.into()),
                &args.msg,
                args.amount.as_u128(),
                &current_account_id,
                handler,
            )
        };

        #[allow(clippy::used_underscore_binding)]
        let amount_to_return = if let Err(_err) = &result {
            sdk::log!("Error in ft_on_transfer: {_err:?}");
            // An error occurred, so we need to return the amount of tokens to the sender.
            args.amount.as_u128()
        } else {
            // Everything is ok, so return 0.
            0
        };

        let output = crate::prelude::format!("\"{amount_to_return}\"");
        io.return_output(output.as_bytes());

        // In case of an error, we just return Ok(None) to avoid a panic in the contract. It's ok
        // because in case of an error, we already returned the amount of tokens to the sender.
        Ok(result.unwrap_or(None))
    })
}

#[named]
pub fn mt_on_transfer<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<Option<SubmitResult>, ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        let current_account_id = env.current_account_id();
        let predecessor_account_id = env.predecessor_account_id();
        let mut engine: Engine<_, _> = Engine::new_with_state(
            state,
            predecessor_address(&predecessor_account_id),
            current_account_id.clone(),
            io,
            env,
        );

        sdk::log!("Call mt_on_transfer");

        let args: MtOnTransferArgs = read_json_args(&io)?;
        let (token_id, amount) = args.single_token().ok_or(errors::ERR_ARGS)?;
        let token = BridgedToken::Nep245(Nep245Token {
            account_id: predecessor_account_id,
            token_id: token_id.into(),
        });
        // Propagate failures so the contract boundary aborts the receipt and
        // the NEP-245 resolver refunds the token.
        let result = engine.receive_erc20_tokens(
            &token,
            &args.msg,
            amount.as_u128(),
            &current_account_id,
            handler,
        )?;

        // The whole amount was used. There is one "0" since we operate with ONE token only.
        io.return_output(br#"["0"]"#);

        Ok(result)
    })
}

#[named]
pub fn deploy_erc20_token<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<PromiseOrValue<Address>, ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
        require_running(&state::get_state(&io)?)?;
        let bytes = io.read_input().to_vec();
        let args =
            DeployErc20TokenArgs::deserialize(&bytes).map_err(|_| errors::ERR_BORSH_DESERIALIZE)?;

        let (token, method, args) = match args {
            DeployErc20TokenArgs::Legacy(nep141) => {
                let token = BridgedToken::Nep141(nep141.into());
                let address = engine::deploy_erc20_token(token, None, io, env, handler)?;

                io.return_output(
                    &borsh::to_vec(address.as_bytes()).map_err(|_| errors::ERR_SERIALIZE)?,
                );

                return Ok(PromiseOrValue::Value(address));
            }
            DeployErc20TokenArgs::WithMetadata(nep141) => (
                BridgedToken::Nep141(nep141.into()),
                "ft_metadata",
                Vec::new(),
            ),
            DeployErc20TokenArgs::Nep245(nep245) => {
                // Serialized by `serde_json`, so the token id is escaped properly.
                let args =
                    serde_json::to_vec(&serde_json::json!({ "token_ids": [&nep245.token_id] }))
                        .map_err(|_| errors::ERR_SERIALIZE)?;
                (
                    BridgedToken::Nep245(nep245),
                    "mt_metadata_base_by_token_id",
                    args,
                )
            }
        };

        let base = PromiseCreateArgs {
            target_account_id: token.account_id().clone(),
            method: method.to_string(),
            args,
            attached_balance: ZERO_YOCTO,
            attached_gas: READ_PROMISE_ATTACHED_GAS,
        };
        let callback = PromiseCreateArgs {
            target_account_id: env.current_account_id(),
            method: "deploy_erc20_token_callback".to_string(),
            args: borsh::to_vec(&token).map_err(|_| errors::ERR_SERIALIZE)?,
            attached_balance: ZERO_YOCTO,
            attached_gas: DEPLOY_ERC20_TOKEN_CALLBACK_ATTACHED_GAS,
        };
        // Safe because these promises are read-only calls to the main engine contract
        // and this transaction could be executed by the owner of the contract only.
        let promise_args = PromiseWithCallbackArgs { base, callback };
        let promise_id = handler.promise_create_with_callback(&promise_args);

        handler.promise_return(promise_id);

        Ok(PromiseOrValue::Promise(promise_args))
    })
}

#[named]
pub fn deploy_erc20_token_callback<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<Address, ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
        require_running(&state::get_state(&io)?)?;
        env.assert_private_call()?;

        if handler.promise_results_count() != 1 {
            return Err(errors::ERR_PROMISE_COUNT.into());
        }

        // Callbacks scheduled before NEP-245 support carry an untagged NEP-141 `AccountId`.
        let token = BridgedToken::deserialize(&io.read_input().to_vec())
            .map_err(|_| BorshDeserializeError)?;
        let erc20_metadata = match (&token, handler.promise_result(0)) {
            (BridgedToken::Nep141(_), Some(PromiseResult::Successful(bytes))) => {
                Some(serde_json::from_slice(&bytes).map_err(ParseArgsError::from)?)
            }
            (BridgedToken::Nep245(_), Some(PromiseResult::Successful(bytes))) => {
                let mut metadata = serde_json::from_slice::<Vec<Option<Nep245Metadata>>>(&bytes)
                    .map_err(ParseArgsError::from)?;
                // One token ID is requested, so the response may contain at most one entry.
                let metadata = match metadata.as_mut_slice() {
                    [] => None,
                    [metadata] => metadata.take(),
                    _ => return Err(errors::ERR_ARGS.into()),
                };

                metadata
                    .map(TryFrom::try_from)
                    .transpose()
                    .map_err(|_| errors::ERR_ARGS)?
            }
            // Metadata is mandatory for NEP-141 tokens and optional for NEP-245 ones.
            (BridgedToken::Nep141(_), _) => {
                return Err(errors::ERR_GETTING_ERC20_FROM_NEP141.into());
            }
            (BridgedToken::Nep245(_), _) => None,
        };

        let address = engine::deploy_erc20_token(token, erc20_metadata, io, env, handler)?;

        io.return_output(&borsh::to_vec(address.as_bytes()).map_err(|_| errors::ERR_SERIALIZE)?);
        Ok(address)
    })
}

#[named]
pub fn exit_to_near_precompile_callback<I: IO + Copy, E: Env, H: PromiseHandler>(
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

        let args = ExitToNearPrecompileCallbackArgs::deserialize(&io.read_input().to_vec())
            .map_err(|_| BorshDeserializeError)?;
        let result = handler.promise_result(0);

        if matches!(result, Some(PromiseResult::Successful(_)))
            && args.transfer_call == TransferCall::None
            && let Some(args) = &args.transfer_near
        {
            let action = PromiseAction::Transfer {
                amount: Yocto::new(args.amount),
            };
            let promise = PromiseBatchAction {
                target_account_id: args.target_account_id.clone(),
                actions: vec![action],
            };

            // Safety: this call is safe because it comes from the exit to near precompile, not users.
            // The call is to transfer the unwrapped wNEAR tokens.
            let promise_id = handler.promise_create_batch(&promise);
            handler.promise_return(promise_id);

            return Ok(None);
        }

        // Refund arguments exist only if the precompile was built with `error_refund`.
        let maybe_result = if let Some(args) = resolve_exit_to_near_refund(&args, result.as_ref())?
        {
            let refund_result = engine::refund_on_error(io, env, state, &args, handler)?;

            if !refund_result.status.is_ok() {
                return Err(errors::ERR_REFUND_FAILURE.into());
            }

            Some(refund_result)
        } else {
            None
        };

        Ok(maybe_result)
    })
}

/// Resolve a refund from the final transfer result. Shared with standalone's EVM representation.
pub fn resolve_exit_to_near_refund(
    args: &ExitToNearPrecompileCallbackArgs,
    result: Option<&PromiseResult>,
) -> Result<Option<RefundCallArgs>, ContractError> {
    let Some(refund) = &args.refund else {
        return Ok(None);
    };

    match args.transfer_call {
        TransferCall::None => match result {
            Some(PromiseResult::Successful(_)) => Ok(None),
            Some(PromiseResult::Failed) => Ok(Some(refund.clone())),
            _ => Err(errors::ERR_TRANSFER_CALL_FAILED.into()),
        },
        TransferCall::Nep141 | TransferCall::Nep245 => {
            // A failed resolver may follow a committed transfer. Its failure alone does not
            // prove that any tokens returned to the engine's NEAR account.
            let Some(PromiseResult::Successful(bytes)) = result else {
                return Err(errors::ERR_TRANSFER_CALL_FAILED.into());
            };

            if bytes.len() > MAX_TRANSFER_RESULT_LENGTH {
                return Err(errors::ERR_TRANSFER_CALL_RESULT.into());
            }

            let used = if args.transfer_call == TransferCall::Nep141 {
                serde_json::from_slice::<Balance>(bytes)
                    .map_err(|_| errors::ERR_TRANSFER_CALL_RESULT)?
            } else {
                serde_json::from_slice::<[Balance; 1]>(bytes)
                    .map(|[used]| used)
                    .map_err(|_| errors::ERR_TRANSFER_CALL_RESULT)?
            };
            let amount = U256::from_big_endian(&refund.amount);

            if amount > U256::from(u128::MAX) || U256::from(used.as_u128()) > amount {
                return Err(errors::ERR_TRANSFER_CALL_RESULT.into());
            }

            let unused = amount - U256::from(used.as_u128());

            if unused.is_zero() {
                return Ok(None);
            }

            Ok(Some(RefundCallArgs {
                amount: unused.to_big_endian(),
                ..refund.clone()
            }))
        }
    }
}

pub fn ft_transfer<I: IO + Copy + PromiseHandler, E: Env>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    env.assert_one_yocto()?;
    let args = read_json_args(&io).and_then(|args: FtTransferArgs| {
        serde_json::to_vec(&(
            env.predecessor_account_id(),
            args.receiver_id,
            args.amount,
            args.memo,
        ))
        .map_err(Into::<ParseArgsError>::into)
    })?;

    return_promise(io, env, "engine_ft_transfer", args, ONE_YOCTO)
}

pub fn ft_transfer_call<I: IO + Copy + PromiseHandler, E: Env>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    // Check is payable
    env.assert_one_yocto()?;
    let args = read_json_args(&io).and_then(|args: FtTransferCallArgs| {
        serde_json::to_vec(&(
            env.predecessor_account_id(),
            args.receiver_id,
            args.amount,
            args.memo,
            args.msg,
        ))
        .map_err(Into::<ParseArgsError>::into)
    })?;

    return_promise(io, env, "engine_ft_transfer_call", args, ONE_YOCTO)
}

pub fn storage_deposit<I: IO + Copy + PromiseHandler, E: Env>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    let args = read_json_args(&io).and_then(|args: StorageDepositArgs| {
        serde_json::to_vec(&(
            env.predecessor_account_id(),
            args.account_id,
            args.registration_only,
        ))
        .map_err(Into::<ParseArgsError>::into)
    })?;

    return_promise(
        io,
        env,
        "engine_storage_deposit",
        args,
        Yocto::new(env.attached_deposit()),
    )
}

pub fn storage_unregister<I: IO + Copy + PromiseHandler, E: Env>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    env.assert_one_yocto()?;

    let args = read_json_args(&io).and_then(|args: StorageUnregisterArgs| {
        serde_json::to_vec(&(env.predecessor_account_id(), args.force))
            .map_err(Into::<ParseArgsError>::into)
    })?;

    return_promise(io, env, "engine_storage_unregister", args, ONE_YOCTO)
}

pub fn storage_withdraw<I: IO + PromiseHandler + Copy, E: Env>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    require_running(&state::get_state(&io)?)?;
    env.assert_one_yocto()?;

    let args = read_json_args(&io).and_then(|args: StorageWithdrawArgs| {
        serde_json::to_vec(&(env.predecessor_account_id(), args.amount))
            .map_err(Into::<ParseArgsError>::into)
    })?;

    return_promise(io, env, "engine_storage_withdraw", args, ONE_YOCTO)
}

pub fn storage_balance_of<I: IO + Copy + PromiseHandler + Env>(io: I) -> Result<(), ContractError> {
    let args = io.read_input().to_vec();
    return_promise(io, &io, "storage_balance_of", args, ZERO_YOCTO)
}

pub fn ft_total_eth_supply_on_near<I: IO + Copy + PromiseHandler + Env>(
    io: I,
) -> Result<(), ContractError> {
    return_promise(io, &io, "ft_total_supply", Vec::new(), ZERO_YOCTO)
}

pub fn ft_balance_of<I: IO + Copy + PromiseHandler + Env>(io: I) -> Result<(), ContractError> {
    let args = io.read_input().to_vec();
    return_promise(io, &io, "ft_balance_of", args, ZERO_YOCTO)
}

/// Returns the balance of the given address in the base tokens. The method returns the
/// same value as the`get_balance` but in JSON.
pub fn ft_balance_of_eth<I: IO + Copy>(mut io: I) -> Result<(), ContractError> {
    let address: Address = io.read_input_borsh()?;
    let balance = engine::get_balance(&io, &address);

    io.return_output(&serde_json::to_vec(&balance).map_err(Into::<ParseArgsError>::into)?);

    Ok(())
}

#[named]
pub fn set_erc20_metadata<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<SubmitResult, ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        // TODO: Define special role for this transaction. Potentially via multisig?
        let is_private = env.assert_private_call();
        if is_private.is_err() {
            require_owner_only(&state, &env.predecessor_account_id())?;
        }

        let args: SetErc20MetadataArgs = serde_json::from_slice(&io.read_input().to_vec())
            .map_err(Into::<ParseArgsError>::into)?;
        let current_account_id = env.current_account_id();
        let mut engine: Engine<_, E, AuroraModExp> = Engine::new_with_state(
            state,
            predecessor_address(&current_account_id),
            current_account_id,
            io,
            env,
        );
        let result = engine.set_erc20_metadata(&args.erc20_identifier, args.metadata, handler)?;

        Ok(result)
    })
}

pub fn get_erc20_metadata<I: IO + Copy, E: Env>(mut io: I, env: &E) -> Result<(), ContractError> {
    let erc20_identifier =
        serde_json::from_slice(&io.read_input().to_vec()).map_err(Into::<ParseArgsError>::into)?;
    let state = state::get_state(&io)?;
    let current_account_id = env.current_account_id();
    let engine: Engine<_, E, AuroraModExp> = Engine::new_with_state(
        state,
        predecessor_address(&env.predecessor_account_id()),
        current_account_id,
        io,
        env,
    );
    let metadata = engine.get_erc20_metadata(&erc20_identifier)?;

    io.return_output(&serde_json::to_vec(&metadata).map_err(|_| errors::ERR_SERIALIZE)?);
    Ok(())
}

#[named]
pub fn set_eth_connector_contract_account<I: IO + Copy, E: Env>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |io| {
        let state = state::get_state(&io)?;
        require_running(&state)?;
        let is_private = env.assert_private_call();

        if is_private.is_err() {
            require_owner_only(&state, &env.predecessor_account_id())?;
        }

        let args: SetEthConnectorContractAccountArgs = io.read_input_borsh()?;

        set_connector_account_id(io, &args.account);
        set_connector_withdraw_serialization_type(io, &args.withdraw_serialize_type);

        Ok(())
    })
}

pub fn get_eth_connector_contract_account<I: IO + Copy>(mut io: I) -> Result<(), ContractError> {
    let account_id = get_connector_account_id(&io)?;
    let data = borsh::to_vec(&account_id).unwrap();

    io.return_output(&data);

    Ok(())
}

pub fn ft_metadata<I: IO + Copy + PromiseHandler, E: Env>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    return_promise(io, env, "ft_metadata", Vec::new(), ZERO_YOCTO)
}

pub fn mirror_erc20_token<I: IO + Env + Copy, H: PromiseHandler>(
    io: I,
    handler: &mut H,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    // TODO: Add an admin access list of accounts allowed to do it.
    require_owner_only(&state, &io.predecessor_account_id())?;

    let input = io.read_input().to_vec();
    let args =
        MirrorErc20TokenArgs::deserialize(&input).map_err(|_| errors::ERR_BORSH_DESERIALIZE)?;
    let (method, address_args) = match &args.token {
        BridgedToken::Nep141(token) => ("get_erc20_from_nep141", borsh::to_vec(&token.0)),
        BridgedToken::Nep245(token) => ("get_erc20_from_nep245", borsh::to_vec(token)),
    };

    // We can't use a batch of actions here, since we need to get responses from both
    // view transactions in the `mirror_erc20_token_callback` callback.
    let promises = vec![
        PromiseCreateArgs {
            target_account_id: args.contract_id.clone(),
            method: method.to_string(),
            args: address_args.map_err(|_| errors::ERR_SERIALIZE)?,
            attached_balance: Yocto::new(0),
            attached_gas: READ_PROMISE_ATTACHED_GAS,
        },
        PromiseCreateArgs {
            target_account_id: args.contract_id,
            method: "get_erc20_metadata".into(),
            args: serde_json::to_vec(&Erc20Identifier::from(args.token))
                .map_err(|_| errors::ERR_SERIALIZE)?,
            attached_balance: Yocto::new(0),
            attached_gas: READ_PROMISE_ATTACHED_GAS,
        },
    ];

    let callback = PromiseCreateArgs {
        target_account_id: io.current_account_id(),
        method: "mirror_erc20_token_callback".to_string(),
        args: input,
        attached_balance: Yocto::new(0),
        attached_gas: MIRROR_ERC20_TOKEN_CALLBACK_ATTACHED_GAS,
    };
    // Safe because these promises are read-only calls to the main engine contract,
    // and this transaction could be executed by the owner of the contract only.
    let base_id = handler.promise_create_and_combine(&promises);
    let callback_id = handler.promise_attach_callback(base_id, &callback);

    handler.promise_return(callback_id);

    Ok(())
}

#[named]
pub fn mirror_erc20_token_callback<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
        let state = state::get_state(&io)?;

        require_running(&state)?;
        env.assert_private_call()?;

        if handler.promise_results_count() != 2 {
            return Err(errors::ERR_PROMISE_COUNT.into());
        }

        let args = MirrorErc20TokenArgs::deserialize(&io.read_input().to_vec())
            .map_err(|_| errors::ERR_BORSH_DESERIALIZE)?;
        let error = || match args.token {
            BridgedToken::Nep141(_) => errors::ERR_GETTING_ERC20_FROM_NEP141.into(),
            BridgedToken::Nep245(_) => errors::ERR_GETTING_ERC20_FROM_NEP245.into(),
        };
        let erc20_address =
            if let Some(PromiseResult::Successful(bytes)) = handler.promise_result(0) {
                Address::try_from_slice(&bytes)?
            } else {
                return Err(error());
            };

        let erc20_metadata =
            if let Some(PromiseResult::Successful(bytes)) = handler.promise_result(1) {
                serde_json::from_slice(&bytes).map_err(Into::<ParseArgsError>::into)?
            } else {
                return Err(error());
            };

        let address =
            engine::mirror_erc20_token(args, erc20_address, erc20_metadata, io, env, handler)?;

        io.return_output(&borsh::to_vec(address.as_bytes()).map_err(|_| errors::ERR_SERIALIZE)?);

        Ok(())
    })
}

fn construct_contract_key(suffix: EthConnectorStorageId) -> Vec<u8> {
    crate::prelude::bytes_to_key(KeyPrefix::EthConnector, [u8::from(suffix)])
}

pub fn get_connector_account_id<I: IO>(io: &I) -> Result<AccountId, ContractError> {
    io.read_storage(&construct_contract_key(
        EthConnectorStorageId::EthConnectorAccount,
    ))
    .ok_or(errors::ERR_CONNECTOR_STORAGE_KEY_NOT_FOUND)
    .and_then(|x| {
        x.to_value()
            .map_err(|_| errors::ERR_BORSH_DESERIALIZE.as_bytes())
    })
    .map_err(Into::into)
}

pub fn set_connector_account_id<I: IO + Copy>(mut io: I, account_id: &AccountId) {
    io.write_borsh(
        &construct_contract_key(EthConnectorStorageId::EthConnectorAccount),
        account_id,
    );
}

pub fn set_connector_withdraw_serialization_type<I: IO + Copy>(
    mut io: I,
    serialize_type: &WithdrawSerializeType,
) {
    io.write_borsh(
        &construct_contract_key(EthConnectorStorageId::WithdrawSerializationType),
        serialize_type,
    );
}

fn read_json_args<I: IO, T>(io: &I) -> Result<T, ParseArgsError>
where
    T: serde::de::DeserializeOwned,
{
    let bytes = io.read_input().to_vec();
    aurora_engine_types::parameters::engine::parse_json_args(&bytes)
}

// TODO: Return `Result` with an error about lacking of gas instead.
fn calculate_attached_gas<E: Env>(env: &E) -> NearGas {
    let required_gas = env.used_gas().saturating_add(GAS_FOR_PROMISE_CREATION);

    if required_gas >= env.prepaid_gas() {
        NearGas::new(0)
    } else {
        env.prepaid_gas() - required_gas
    }
}

fn return_promise<I: IO + PromiseHandler, E: Env>(
    mut io: I,
    env: &E,
    method: &str,
    args: Vec<u8>,
    deposit: Yocto,
) -> Result<(), ContractError> {
    let promise_args = PromiseCreateArgs {
        target_account_id: get_connector_account_id(&io)?,
        method: method.to_string(),
        args,
        attached_balance: deposit,
        attached_gas: calculate_attached_gas(env),
    };
    let promise_id = io.promise_create_call(&promise_args);

    io.promise_return(promise_id);

    Ok(())
}

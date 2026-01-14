use aurora_engine_modexp::AuroraModExp;
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::{IO, StorageIntermediate};
use aurora_engine_sdk::promise::PromiseHandler;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::borsh::{self, BorshDeserialize};
use aurora_engine_types::parameters::connector::{
    EngineWithdrawCallArgs, Erc20Identifier, Erc20Metadata, ExitToNearPrecompileCallbackArgs,
    FtOnTransferArgs, FtTransferArgs, FtTransferCallArgs, FungibleTokenMetadata,
    MirrorErc20TokenArgs, SetErc20MetadataArgs, SetEthConnectorContractAccountArgs,
    StorageDepositArgs, StorageUnregisterArgs, StorageWithdrawArgs, WithdrawCallArgs,
    WithdrawSerializeType,
};
use aurora_engine_types::parameters::engine::errors::ParseArgsError;
use aurora_engine_types::parameters::engine::{DeployErc20TokenArgs, SubmitResult};
use aurora_engine_types::parameters::{
    PromiseAction, PromiseBatchAction, PromiseCreateArgs, PromiseOrValue, PromiseWithCallbackArgs,
};
use aurora_engine_types::storage::{EthConnectorStorageId, KeyPrefix};
use aurora_engine_types::types::{Address, NearGas, PromiseResult, Yocto};
use function_name::named;

use crate::contract_methods::{
    ContractError, predecessor_address, require_owner_only, require_running,
};
use crate::engine::Engine;
use crate::hashchain::with_hashchain;
use crate::prelude::{ToString, Vec, sdk, vec};
use crate::{engine, errors, state};

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
        require_running(&state::get_state(&io)?)?;
        let current_account_id = env.current_account_id();
        let predecessor_account_id = env.predecessor_account_id();
        let mut engine: Engine<_, _> = Engine::new(
            predecessor_address(&predecessor_account_id),
            current_account_id.clone(),
            io,
            env,
        )?;

        sdk::log!("Call ft_on_transfer");

        let args: FtOnTransferArgs = read_json_args(&io)?;
        let result = if predecessor_account_id == get_connector_account_id(&io)? {
            engine.receive_base_tokens(&args)
        } else {
            engine.receive_erc20_tokens(
                &predecessor_account_id,
                &args,
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

        match args {
            DeployErc20TokenArgs::Legacy(nep141) => {
                let address = engine::deploy_erc20_token(nep141, None, io, env, handler)?;

                io.return_output(
                    &borsh::to_vec(address.as_bytes()).map_err(|_| errors::ERR_SERIALIZE)?,
                );
                Ok(PromiseOrValue::Value(address))
            }
            DeployErc20TokenArgs::WithMetadata(nep141) => {
                let args = borsh::to_vec(&nep141).map_err(|_| errors::ERR_SERIALIZE)?;
                let base = PromiseCreateArgs {
                    target_account_id: nep141,
                    method: "ft_metadata".to_string(),
                    args: vec![],
                    attached_balance: ZERO_YOCTO,
                    attached_gas: READ_PROMISE_ATTACHED_GAS,
                };
                let callback = PromiseCreateArgs {
                    target_account_id: env.current_account_id(),
                    method: "deploy_erc20_token_callback".to_string(),
                    args,
                    attached_balance: ZERO_YOCTO,
                    attached_gas: DEPLOY_ERC20_TOKEN_CALLBACK_ATTACHED_GAS,
                };
                // Safe because these promises are read-only calls to the main engine contract
                // and this transaction could be executed by the owner of the contract only.
                let promise_args = PromiseWithCallbackArgs { base, callback };
                let promise_id = handler.promise_create_with_callback(&promise_args);

                handler.promise_return(promise_id);

                Ok(PromiseOrValue::Promise(promise_args))
            }
        }
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

        let nep141 = io.read_input_borsh()?;
        let erc20_metadata =
            if let Some(PromiseResult::Successful(bytes)) = handler.promise_result(0) {
                serde_json::from_slice::<FungibleTokenMetadata>(&bytes)
                    .map(|metadata| Erc20Metadata {
                        name: metadata.name,
                        symbol: metadata.symbol,
                        decimals: metadata.decimals,
                    })
                    .map_err(Into::<ParseArgsError>::into)?
            } else {
                return Err(errors::ERR_GETTING_ERC20_FROM_NEP141.into());
            };
        let address = engine::deploy_erc20_token(nep141, Some(erc20_metadata), io, env, handler)?;

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

        let args: ExitToNearPrecompileCallbackArgs = io.read_input_borsh()?;

        let maybe_result = if let Some(PromiseResult::Successful(_)) = handler.promise_result(0) {
            if let Some(args) = args.transfer_near {
                let action = PromiseAction::Transfer {
                    amount: Yocto::new(args.amount),
                };
                let promise = PromiseBatchAction {
                    target_account_id: args.target_account_id,
                    actions: vec![action],
                };

                // Safety: this call is safe because it comes from the exit to near precompile, not users.
                // The call is to transfer the unwrapped wNEAR tokens.
                let promise_id = handler.promise_create_batch(&promise);
                handler.promise_return(promise_id);
            }

            None
        } else if let Some(args) = args.refund {
            // Exit call failed; need to refund tokens
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

    return_promise(io, env, "engine_storage_withdraw", args, ZERO_YOCTO)
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
        MirrorErc20TokenArgs::try_from_slice(&input).map_err(|_| errors::ERR_BORSH_DESERIALIZE)?;

    // We can't use a batch of actions here, since we need to get responses from both
    // view transactions in the `mirror_erc20_token_callback` callback.
    let promises = vec![
        PromiseCreateArgs {
            target_account_id: args.contract_id.clone(),
            method: "get_erc20_from_nep141".to_string(),
            args: borsh::to_vec(&args.nep141).map_err(|_| errors::ERR_SERIALIZE)?,
            attached_balance: Yocto::new(0),
            attached_gas: READ_PROMISE_ATTACHED_GAS,
        },
        PromiseCreateArgs {
            target_account_id: args.contract_id,
            method: "get_erc20_metadata".into(),
            args: serde_json::to_vec(&Erc20Identifier::from(args.nep141))
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

        let args: MirrorErc20TokenArgs = io.read_input_borsh()?;
        let erc20_address =
            if let Some(PromiseResult::Successful(bytes)) = handler.promise_result(0) {
                Address::try_from_slice(&bytes)?
            } else {
                return Err(errors::ERR_GETTING_ERC20_FROM_NEP141.into());
            };

        let erc20_metadata =
            if let Some(PromiseResult::Successful(bytes)) = handler.promise_result(1) {
                serde_json::from_slice(&bytes).map_err(Into::<ParseArgsError>::into)?
            } else {
                return Err(errors::ERR_GETTING_ERC20_FROM_NEP141.into());
            };

        let address =
            engine::mirror_erc20_token(args, erc20_address, erc20_metadata, io, env, handler)?;

        io.return_output(&borsh::to_vec(address.as_bytes()).map_err(|_| errors::ERR_SERIALIZE)?);

        Ok(())
    })
}

fn construct_contract_key(suffix: EthConnectorStorageId) -> Vec<u8> {
    crate::prelude::bytes_to_key(KeyPrefix::EthConnector, &[u8::from(suffix)])
}

fn get_connector_account_id<I: IO>(io: &I) -> Result<AccountId, ContractError> {
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

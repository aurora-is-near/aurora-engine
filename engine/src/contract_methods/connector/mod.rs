#![allow(clippy::missing_const_for_fn)]

use crate::contract_methods::{
    predecessor_address, require_owner_only, require_running, ContractError,
};
use crate::engine::Engine;
use crate::hashchain::with_hashchain;
use crate::prelude::{vec, ToString, Vec};
use crate::{engine, state};
use aurora_engine_modexp::AuroraModExp;
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_sdk::promise::PromiseHandler;
use aurora_engine_types::borsh::{BorshDeserialize, BorshSerialize};
use aurora_engine_types::parameters::connector::{
    Erc20Identifier, MirrorErc20TokenArgs, SetErc20MetadataArgs,
};
use aurora_engine_types::parameters::engine::errors::ParseArgsError;
use aurora_engine_types::parameters::engine::{
    DeployErc20TokenArgs, GetErc20FromNep141CallArgs, SubmitResult,
};
use aurora_engine_types::parameters::{
    ExitToNearPrecompileCallbackCallArgs, PromiseAction, PromiseBatchAction,
};
use aurora_engine_types::parameters::{PromiseCreateArgs, PromiseWithCallbackArgs};
use aurora_engine_types::storage::{EthConnectorStorageId, KeyPrefix};
use aurora_engine_types::types::{Address, NearGas, PromiseResult, Yocto};
use function_name::named;

#[cfg(feature = "ext-connector")]
pub use external::{AdminControlled, EthConnector, EthConnectorContract};
#[cfg(not(feature = "ext-connector"))]
pub use internal::{EthConnector, EthConnectorContract};

pub mod admin_controlled;
pub mod deposit_event;
pub mod errors;
#[cfg(feature = "ext-connector")]
pub mod external;
pub mod fungible_token;
#[cfg(not(feature = "ext-connector"))]
pub mod internal;

pub const ERR_NOT_ENOUGH_BALANCE_FOR_FEE: &str = "ERR_NOT_ENOUGH_BALANCE_FOR_FEE";
/// Indicate zero attached balance for promise call
pub const ZERO_ATTACHED_BALANCE: Yocto = Yocto::new(0);
/// Amount of attached gas for read-only promises.
const READ_PROMISE_ATTACHED_GAS: NearGas = NearGas::new(5_000_000_000_000);

/// Create new eth-connector;
pub fn new_eth_connector<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    internal::new_eth_connector(io, env)?;
    #[cfg(feature = "ext-connector")]
    let (_, _) = (io, env);

    Ok(())
}

/// Set eth-connector data.
pub fn set_eth_connector_contract_data<I: IO + Copy, E: Env>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    internal::set_eth_connector_contract_data(io, env)?;
    #[cfg(feature = "ext-connector")]
    let (_, _) = (io, env);

    Ok(())
}

pub fn withdraw<
    #[cfg(not(feature = "ext-connector"))] I: IO + Copy,
    #[cfg(feature = "ext-connector")] I: IO + Copy + PromiseHandler,
    E: Env,
>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    internal::withdraw(io, env)?;
    #[cfg(feature = "ext-connector")]
    external::withdraw(io, env)?;

    Ok(())
}

pub fn deposit<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<Option<PromiseWithCallbackArgs>, ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    let result = internal::deposit(io, env, handler)?;
    #[cfg(feature = "ext-connector")]
    let result = external::deposit(io, env, handler)?;

    Ok(result)
}

pub fn ft_on_transfer<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    internal::ft_on_transfer(io, env, handler)?;
    #[cfg(feature = "ext-connector")]
    external::ft_on_transfer(io, env, handler)?;

    Ok(())
}

#[named]
pub fn deploy_erc20_token<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<Address, ContractError> {
    with_hashchain(io, env, function_name!(), |mut io| {
        require_running(&state::get_state(&io)?)?;
        // AccountId of NEP-141 token on NEAR
        let args: DeployErc20TokenArgs = io.read_input_borsh()?;
        let address = engine::deploy_erc20_token(args, io, env, handler)?;

        io.return_output(
            &address
                .as_bytes()
                .try_to_vec()
                .map_err(|_| crate::errors::ERR_SERIALIZE)?,
        );
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
            return Err(crate::errors::ERR_PROMISE_COUNT.into());
        }

        let args: ExitToNearPrecompileCallbackCallArgs = io.read_input_borsh()?;

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
                let promise_id = unsafe { handler.promise_create_batch(&promise) };
                handler.promise_return(promise_id);
            }

            None
        } else if let Some(args) = args.refund {
            // Exit call failed; need to refund tokens
            let refund_result = engine::refund_on_error(io, env, state, &args, handler)?;

            if !refund_result.status.is_ok() {
                return Err(crate::errors::ERR_REFUND_FAILURE.into());
            }

            Some(refund_result)
        } else {
            None
        };

        Ok(maybe_result)
    })
}

pub fn finish_deposit<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<Option<PromiseWithCallbackArgs>, ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    let result = internal::finish_deposit(io, env, handler)?;
    #[cfg(feature = "ext-connector")]
    let result = external::finish_deposit(io, env, handler)?;

    Ok(result)
}

pub fn ft_transfer<
    #[cfg(not(feature = "ext-connector"))] I: IO + Copy,
    #[cfg(feature = "ext-connector")] I: IO + Copy + PromiseHandler,
    E: Env,
>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    internal::ft_transfer(io, env)?;
    #[cfg(feature = "ext-connector")]
    external::ft_transfer(io, env)?;

    Ok(())
}

pub fn ft_transfer_call<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<Option<PromiseWithCallbackArgs>, ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    let result = internal::ft_transfer_call(io, env, handler)?;
    #[cfg(feature = "ext-connector")]
    let result = external::ft_transfer_call(io, env, handler)?;

    Ok(result)
}

pub fn ft_resolve_transfer<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &H,
) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    internal::ft_resolve_transfer(io, env, handler)?;
    #[cfg(feature = "ext-connector")]
    let (_, _, _) = (io, env, handler);

    Ok(())
}

pub fn storage_deposit<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    internal::storage_deposit(io, env, handler)?;
    #[cfg(feature = "ext-connector")]
    external::storage_deposit(io, env, handler)?;

    Ok(())
}

pub fn storage_unregister<I: IO + Copy, E: Env, H: PromiseHandler>(
    io: I,
    env: &E,
    handler: &mut H,
) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    internal::storage_unregister(io, env, handler)?;
    #[cfg(feature = "ext-connector")]
    external::storage_unregister(io, env, handler)?;

    Ok(())
}

pub fn storage_withdraw<
    #[cfg(not(feature = "ext-connector"))] I: IO + Copy,
    #[cfg(feature = "ext-connector")] I: IO + Copy + PromiseHandler,
    E: Env,
>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    internal::storage_withdraw(io, env)?;
    #[cfg(feature = "ext-connector")]
    external::storage_withdraw(io, env)?;

    Ok(())
}

pub fn storage_balance_of<I: IO + Copy + PromiseHandler>(io: I) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    internal::storage_balance_of(io)?;
    #[cfg(feature = "ext-connector")]
    external::storage_balance_of(io)?;

    Ok(())
}

pub fn set_paused_flags<I: IO + Copy, E: Env>(io: I, env: &E) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    internal::set_paused_flags(io, env)?;
    #[cfg(feature = "ext-connector")]
    let (_, _) = (io, env);

    Ok(())
}

pub fn get_paused_flags<I: IO + Copy + PromiseHandler>(io: I) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    internal::get_paused_flags(io)?;
    #[cfg(feature = "ext-connector")]
    external::get_paused_flags(io)?;

    Ok(())
}

pub fn is_used_proof<I: IO + Copy + PromiseHandler>(io: I) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    internal::is_used_proof(io)?;
    #[cfg(feature = "ext-connector")]
    external::is_used_proof(io)?;

    Ok(())
}

pub fn ft_total_eth_supply_on_near<I: IO + Copy + PromiseHandler>(
    io: I,
) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    internal::ft_total_eth_supply_on_near(io)?;
    #[cfg(feature = "ext-connector")]
    external::ft_total_eth_supply_on_near(io)?;

    Ok(())
}

pub fn ft_total_eth_supply_on_aurora<I: IO + Copy>(io: I) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    EthConnectorContract::init(io)?.ft_total_eth_supply_on_aurora();
    #[cfg(feature = "ext-connector")]
    let _ = io;

    Ok(())
}

pub fn ft_balance_of<I: IO + Copy + PromiseHandler>(io: I) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    internal::ft_balance_of(io)?;
    #[cfg(feature = "ext-connector")]
    external::ft_balance_of(io)?;

    Ok(())
}

pub fn ft_balance_of_eth<I: IO + Copy>(io: I) -> Result<(), ContractError> {
    let args = io.read_input_borsh()?;
    EthConnectorContract::init(io)?.ft_balance_of_eth_on_aurora(&args)?;
    Ok(())
}

#[cfg(not(feature = "ext-connector"))]
pub fn get_accounts_counter<I: IO + Copy>(io: I) -> Result<(), ContractError> {
    internal::get_accounts_counter(io)?;
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
        predecessor_address(&current_account_id),
        current_account_id,
        io,
        env,
    );
    let metadata = engine.get_erc20_metadata(&erc20_identifier)?;

    io.return_output(&serde_json::to_vec(&metadata).map_err(|_| crate::errors::ERR_SERIALIZE)?);
    Ok(())
}

pub fn set_eth_connector_contract_account<I: IO + Copy, E: Env>(
    io: I,
    env: &E,
) -> Result<(), ContractError> {
    #[cfg(feature = "ext-connector")]
    external::set_eth_connector_account_id(io, env)?;
    #[cfg(not(feature = "ext-connector"))]
    let (_, _) = (io, env);

    Ok(())
}

pub fn get_eth_connector_contract_account<I: IO + Copy>(io: I) -> Result<(), ContractError> {
    #[cfg(feature = "ext-connector")]
    external::get_eth_connector_account_id(io)?;
    #[cfg(not(feature = "ext-connector"))]
    let _ = io;

    Ok(())
}

pub fn ft_metadata<
    #[cfg(not(feature = "ext-connector"))] I: IO + Copy,
    #[cfg(feature = "ext-connector")] I: IO + Copy + PromiseHandler,
>(
    io: I,
) -> Result<(), ContractError> {
    #[cfg(not(feature = "ext-connector"))]
    internal::ft_metadata(io)?;
    #[cfg(feature = "ext-connector")]
    external::ft_metadata(io)?;

    Ok(())
}

pub fn mirror_erc20_token<I: IO + Env + Copy, H: PromiseHandler>(
    io: I,
    handler: &mut H,
) -> Result<(), ContractError> {
    let state = state::get_state(&io)?;
    require_running(&state)?;
    // TODO: Add an admin access list of accounts allowed to do it.
    require_owner_only(&state, &io.predecessor_account_id())?;

    if !crate::contract_methods::silo::is_silo_mode_on(&io) {
        return Err(crate::errors::ERR_ALLOWED_IN_SILO_MODE_ONLY.into());
    }

    let input = io.read_input().to_vec();
    let args = MirrorErc20TokenArgs::try_from_slice(&input)
        .map_err(|_| crate::errors::ERR_BORSH_DESERIALIZE)?;

    let promise = vec![
        PromiseCreateArgs {
            target_account_id: args.contract_id.clone(),
            method: "get_erc20_from_nep141".to_string(),
            args: GetErc20FromNep141CallArgs {
                nep141: args.nep141.clone(),
            }
            .try_to_vec()
            .map_err(|_| crate::errors::ERR_SERIALIZE)?,
            attached_balance: Yocto::new(0),
            attached_gas: READ_PROMISE_ATTACHED_GAS,
        },
        PromiseCreateArgs {
            target_account_id: args.contract_id,
            method: "get_erc20_metadata".into(),
            args: serde_json::to_vec(&Erc20Identifier::from(args.nep141))
                .map_err(|_| crate::errors::ERR_SERIALIZE)?,
            attached_balance: Yocto::new(0),
            attached_gas: READ_PROMISE_ATTACHED_GAS,
        },
    ];

    let callback = PromiseCreateArgs {
        target_account_id: io.current_account_id(),
        method: "mirror_erc20_token_callback".to_string(),
        args: input,
        attached_balance: Yocto::new(0),
        attached_gas: READ_PROMISE_ATTACHED_GAS,
    };
    // Safe because these promises are read-only calls to the main engine contract
    // and this transaction could be executed by the owner of the contract only.
    let promise_id = unsafe {
        let promise_id = handler.promise_create_and_combine(&promise);
        handler.promise_attach_callback(promise_id, &callback)
    };

    handler.promise_return(promise_id);

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
            return Err(crate::errors::ERR_PROMISE_COUNT.into());
        }

        let args: MirrorErc20TokenArgs = io.read_input_borsh()?;
        let erc20_address =
            if let Some(PromiseResult::Successful(bytes)) = handler.promise_result(0) {
                Address::try_from_slice(&bytes)?
            } else {
                return Err(crate::errors::ERR_GETTING_ERC20_FROM_NEP141.into());
            };

        let erc20_metadata =
            if let Some(PromiseResult::Successful(bytes)) = handler.promise_result(1) {
                serde_json::from_slice(&bytes).map_err(Into::<ParseArgsError>::into)?
            } else {
                return Err(crate::errors::ERR_GETTING_ERC20_FROM_NEP141.into());
            };

        let address =
            engine::mirror_erc20_token(args, erc20_address, erc20_metadata, io, env, handler)?;

        io.return_output(
            &address
                .as_bytes()
                .try_to_vec()
                .map_err(|_| crate::errors::ERR_SERIALIZE)?,
        );

        Ok(())
    })
}

fn construct_contract_key(suffix: EthConnectorStorageId) -> Vec<u8> {
    crate::prelude::bytes_to_key(KeyPrefix::EthConnector, &[u8::from(suffix)])
}

fn get_contract_data<T: BorshDeserialize, I: IO>(
    io: &I,
    suffix: EthConnectorStorageId,
) -> Result<T, errors::StorageReadError> {
    io.read_storage(&construct_contract_key(suffix))
        .ok_or(errors::StorageReadError::KeyNotFound)
        .and_then(|x| {
            x.to_value()
                .map_err(|_| errors::StorageReadError::BorshDeserialize)
        })
}

#[cfg(any(not(feature = "ext-connector"), test))]
#[must_use]
fn proof_key(proof: &aurora_engine_types::parameters::connector::Proof) -> crate::prelude::String {
    let mut data = proof.log_index.try_to_vec().unwrap();
    data.extend(proof.receipt_index.try_to_vec().unwrap());
    data.extend(proof.header_data.clone());
    aurora_engine_sdk::sha256(&data)
        .0
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::proof_key;
    use crate::contract_methods::connector::deposit_event::{
        DepositedEvent, TokenMessageData, DEPOSITED_EVENT,
    };
    use aurora_engine_types::parameters::connector::{LogEntry, Proof};
    use aurora_engine_types::types::{make_address, Address, Fee, NEP141Wei, Wei};
    use aurora_engine_types::{H160, U256};

    const ETH_CUSTODIAN_ADDRESS: Address =
        make_address(0xd045f7e1, 0x9b2488924b97f9c145b5e51d0d895a65);

    #[test]
    fn test_proof_key_generates_successfully() {
        let recipient_address = Address::new(H160([22u8; 20]));
        let deposit_amount = Wei::new_u64(123_456_789);
        let proof = create_proof(recipient_address, deposit_amount);

        let expected_key =
            "1297721518512077871939115641114233180253108247225100248224214775219368216419218177247";
        let actual_key = proof_key(&proof);

        assert_eq!(expected_key, actual_key);
    }

    fn create_proof(recipient_address: Address, deposit_amount: Wei) -> Proof {
        let eth_custodian_address = ETH_CUSTODIAN_ADDRESS;

        let fee = Fee::new(NEP141Wei::new(0));
        let message = ["aurora", ":", recipient_address.encode().as_str()].concat();
        let token_message_data: TokenMessageData =
            TokenMessageData::parse_event_message_and_prepare_token_message_data(&message, fee)
                .unwrap();

        let deposit_event = DepositedEvent {
            eth_custodian_address,
            sender: Address::new(H160([0u8; 20])),
            token_message_data,
            amount: NEP141Wei::new(deposit_amount.raw().as_u128()),
            fee,
        };

        let event_schema = ethabi::Event {
            name: DEPOSITED_EVENT.into(),
            inputs: DepositedEvent::event_params(),
            anonymous: false,
        };
        let log_entry = LogEntry {
            address: eth_custodian_address.raw(),
            topics: vec![
                event_schema.signature(),
                // the sender is not important
                crate::prelude::H256::zero(),
            ],
            data: ethabi::encode(&[
                ethabi::Token::String(message),
                ethabi::Token::Uint(U256::from(deposit_event.amount.as_u128())),
                ethabi::Token::Uint(U256::from(deposit_event.fee.as_u128())),
            ]),
        };

        Proof {
            log_index: 1,
            // Only this field matters for the purpose of this test
            log_entry_data: rlp::encode(&log_entry).to_vec(),
            receipt_index: 1,
            receipt_data: Vec::new(),
            header_data: Vec::new(),
            proof: Vec::new(),
        }
    }
}

use aurora_engine::pausables::{
    EnginePrecompilesPauser, PausedPrecompilesManager, PrecompileFlags,
};
use aurora_engine::{
    engine,
    parameters::{self, SubmitResult},
    silo, state, xcc,
};
use aurora_engine_modexp::ModExpAlgorithm;
use aurora_engine_sdk::env::{self, Env, DEFAULT_PREPAID_GAS};
use aurora_engine_standalone_nep141_legacy::legacy_connector;
use aurora_engine_transactions::EthTransactionKind;
use aurora_engine_types::{
    account_id::AccountId,
    borsh::BorshDeserialize,
    parameters::{silo as silo_params, PromiseWithCallbackArgs},
    types::{Address, Yocto},
    H256,
};
use std::{io, str::FromStr};

pub mod types;

use crate::engine_state::EngineStateAccess;
use crate::{error::ParseTransactionKindError, BlockMetadata, Diff, Storage};
use types::{Message, TransactionKind, TransactionKindTag, TransactionMessage};

/// Try to parse an Aurora transaction from raw information available in a Near action
/// (method name, input bytes, data returned from promises).
#[allow(clippy::too_many_lines)]
pub fn parse_transaction_kind(
    method_name: &str,
    bytes: Vec<u8>,
    promise_data: &[Option<Vec<u8>>],
) -> Result<TransactionKind, ParseTransactionKindError> {
    let tx_kind_tag = TransactionKindTag::from_str(method_name).map_err(|_| {
        ParseTransactionKindError::UnknownMethodName {
            name: method_name.into(),
        }
    })?;
    let f = |e: io::Error| ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e));

    let tx_kind = match tx_kind_tag {
        TransactionKindTag::Submit => {
            let eth_tx = EthTransactionKind::try_from(bytes.as_slice()).map_err(|e| {
                ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
            })?;
            TransactionKind::Submit(eth_tx)
        }
        TransactionKindTag::SubmitWithArgs => {
            let args = parameters::SubmitArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SubmitWithArgs(args)
        }
        TransactionKindTag::Call => {
            let call_args = parameters::CallArgs::deserialize(&bytes).ok_or_else(|| {
                ParseTransactionKindError::failed_deserialization::<io::Error>(tx_kind_tag, None)
            })?;
            TransactionKind::Call(call_args)
        }
        TransactionKindTag::PausePrecompiles => {
            let args = parameters::PausePrecompilesCallArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::PausePrecompiles(args)
        }
        TransactionKindTag::ResumePrecompiles => {
            let args = parameters::PausePrecompilesCallArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::ResumePrecompiles(args)
        }
        TransactionKindTag::SetOwner => {
            let args = parameters::SetOwnerArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetOwner(args)
        }
        TransactionKindTag::Deploy => TransactionKind::Deploy(bytes),
        TransactionKindTag::DeployErc20 => {
            let deploy_args =
                parameters::DeployErc20TokenArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::DeployErc20(deploy_args)
        }
        TransactionKindTag::FtOnTransfer => {
            let transfer_args: parameters::NEP141FtOnTransferArgs =
                serde_json::from_slice(bytes.as_slice()).map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;

            TransactionKind::FtOnTransfer(transfer_args)
        }
        TransactionKindTag::Deposit => TransactionKind::Deposit(bytes),
        TransactionKindTag::FtTransferCall => {
            let transfer_args: parameters::TransferCallCallArgs =
                serde_json::from_slice(bytes.as_slice()).map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;

            TransactionKind::FtTransferCall(transfer_args)
        }
        TransactionKindTag::FinishDeposit => {
            let args = parameters::FinishDepositCallArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::FinishDeposit(args)
        }
        TransactionKindTag::ResolveTransfer => {
            let args = parameters::ResolveTransferCallArgs::try_from_slice(&bytes).map_err(f)?;
            let promise_result = promise_data
                .first()
                .and_then(Option::as_ref)
                .map_or(aurora_engine_types::types::PromiseResult::Failed, |bytes| {
                    aurora_engine_types::types::PromiseResult::Successful(bytes.clone())
                });
            TransactionKind::ResolveTransfer(args, promise_result)
        }
        TransactionKindTag::FtTransfer => {
            let args: parameters::TransferCallArgs = serde_json::from_slice(bytes.as_slice())
                .map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;

            TransactionKind::FtTransfer(args)
        }
        TransactionKindTag::Withdraw => {
            let args = aurora_engine_types::parameters::WithdrawCallArgs::try_from_slice(&bytes)
                .map_err(f)?;
            TransactionKind::Withdraw(args)
        }
        TransactionKindTag::StorageDeposit => {
            let args: parameters::StorageDepositCallArgs = serde_json::from_slice(bytes.as_slice())
                .map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;

            TransactionKind::StorageDeposit(args)
        }
        TransactionKindTag::StorageUnregister => {
            let json_args: serde_json::Value =
                serde_json::from_slice(bytes.as_slice()).map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;
            let force = json_args
                .as_object()
                .and_then(|x| x.get("force"))
                .and_then(serde_json::Value::as_bool);

            TransactionKind::StorageUnregister(force)
        }
        TransactionKindTag::StorageWithdraw => {
            let args: parameters::StorageWithdrawCallArgs =
                serde_json::from_slice(bytes.as_slice()).map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;

            TransactionKind::StorageWithdraw(args)
        }
        TransactionKindTag::SetPausedFlags => {
            let args = parameters::PauseEthConnectorCallArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetPausedFlags(args)
        }
        TransactionKindTag::RegisterRelayer => {
            let address = Address::try_from_slice(&bytes).map_err(|e| {
                ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
            })?;
            TransactionKind::RegisterRelayer(address)
        }
        TransactionKindTag::RefundOnError => match promise_data.first().and_then(Option::as_ref) {
            None => TransactionKind::RefundOnError(None),
            Some(_) => {
                let args = aurora_engine_types::parameters::RefundCallArgs::try_from_slice(&bytes)
                    .map_err(f)?;
                TransactionKind::RefundOnError(Some(args))
            }
        },
        TransactionKindTag::SetConnectorData => {
            let args = parameters::SetContractDataCallArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetConnectorData(args)
        }
        TransactionKindTag::NewConnector => {
            let args = parameters::InitCallArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::NewConnector(args)
        }
        TransactionKindTag::NewEngine => {
            let args = parameters::NewCallArgs::deserialize(&bytes).map_err(|e| {
                ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
            })?;
            TransactionKind::NewEngine(args)
        }
        TransactionKindTag::FactoryUpdate => TransactionKind::FactoryUpdate(bytes),
        TransactionKindTag::FactoryUpdateAddressVersion => {
            let args = xcc::AddressVersionUpdateArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::FactoryUpdateAddressVersion(args)
        }
        TransactionKindTag::FactorySetWNearAddress => {
            let address = Address::try_from_slice(&bytes).map_err(|e| {
                ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
            })?;
            TransactionKind::FactorySetWNearAddress(address)
        }
        TransactionKindTag::SetUpgradeDelayBlocks => {
            let args = parameters::SetUpgradeDelayBlocksArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetUpgradeDelayBlocks(args)
        }
        TransactionKindTag::FundXccSubAccount => {
            let args = xcc::FundXccArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::FundXccSubAccount(args)
        }
        TransactionKindTag::PauseContract => TransactionKind::PauseContract,
        TransactionKindTag::ResumeContract => TransactionKind::ResumeContract,
        TransactionKindTag::SetKeyManager => {
            let args = parameters::RelayerKeyManagerArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetKeyManager(args)
        }
        TransactionKindTag::AddRelayerKey => {
            let args = parameters::RelayerKeyArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::AddRelayerKey(args)
        }
        TransactionKindTag::RemoveRelayerKey => {
            let args = parameters::RelayerKeyArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::RemoveRelayerKey(args)
        }
        TransactionKindTag::SetFixedGasCost => {
            let args = silo_params::FixedGasCostArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetFixedGasCost(args)
        }
        TransactionKindTag::SetSiloParams => {
            let args: Option<silo_params::SiloParamsArgs> =
                BorshDeserialize::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetSiloParams(args)
        }
        TransactionKindTag::SetWhitelistStatus => {
            let args = silo_params::WhitelistStatusArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetWhitelistStatus(args)
        }
        TransactionKindTag::AddEntryToWhitelist => {
            let args = silo_params::WhitelistArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::AddEntryToWhitelist(args)
        }
        TransactionKindTag::AddEntryToWhitelistBatch => {
            let args: Vec<silo_params::WhitelistArgs> =
                BorshDeserialize::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::AddEntryToWhitelistBatch(args)
        }
        TransactionKindTag::RemoveEntryFromWhitelist => {
            let args = silo_params::WhitelistArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::RemoveEntryFromWhitelist(args)
        }
        TransactionKindTag::SetEthConnectorContractAccount => {
            let args = parameters::SetEthConnectorContractAccountArgs::try_from_slice(&bytes)
                .map_err(f)?;
            TransactionKind::SetEthConnectorContractAccount(args)
        }
        TransactionKindTag::DisableLegacyNEP141 => TransactionKind::DisableLegacyNEP141,
        TransactionKindTag::Unknown => {
            return Err(ParseTransactionKindError::UnknownMethodName {
                name: method_name.into(),
            });
        }
    };
    Ok(tx_kind)
}

pub fn consume_message<M: ModExpAlgorithm + 'static>(
    storage: &mut Storage,
    message: Message,
) -> Result<ConsumeMessageOutcome, crate::Error> {
    match message {
        Message::Block(block_message) => {
            let block_hash = block_message.hash;
            let block_height = block_message.height;
            let block_metadata = block_message.metadata;
            storage
                .set_block_data(block_hash, block_height, &block_metadata)
                .map_err(crate::Error::Rocksdb)?;
            Ok(ConsumeMessageOutcome::BlockAdded)
        }

        Message::Transaction(transaction_message) => {
            // Failed transactions have no impact on the state of our database.
            if !transaction_message.succeeded {
                return Ok(ConsumeMessageOutcome::FailedTransactionIgnored);
            }

            let transaction_position = transaction_message.position;
            let block_hash = transaction_message.block_hash;
            let block_height = storage.get_block_height_by_hash(block_hash)?;
            let block_metadata = storage.get_block_metadata(block_hash)?;
            let engine_account_id = storage.get_engine_account_id()?;

            let (tx_hash, diff, result) = storage
                .with_engine_access(block_height, transaction_position, &[], |io| {
                    execute_transaction::<M>(
                        transaction_message.as_ref(),
                        block_height,
                        &block_metadata,
                        engine_account_id,
                        io,
                    )
                })
                .result;
            match result.as_ref() {
                Err(_) | Ok(Some(TransactionExecutionResult::Submit(Err(_)))) => (), // do not persist if Engine encounters an error
                _ => storage.set_transaction_included(tx_hash, &transaction_message, &diff)?,
            }
            let outcome = TransactionIncludedOutcome {
                hash: tx_hash,
                info: *transaction_message,
                diff,
                maybe_result: result,
            };
            Ok(ConsumeMessageOutcome::TransactionIncluded(Box::new(
                outcome,
            )))
        }
    }
}

pub fn execute_transaction_message<M: ModExpAlgorithm + 'static>(
    storage: &Storage,
    transaction_message: TransactionMessage,
) -> Result<TransactionIncludedOutcome, crate::Error> {
    let transaction_position = transaction_message.position;
    let block_hash = transaction_message.block_hash;
    let block_height = storage.get_block_height_by_hash(block_hash)?;
    let block_metadata = storage.get_block_metadata(block_hash)?;
    let engine_account_id = storage.get_engine_account_id()?;
    let result = storage.with_engine_access(block_height, transaction_position, &[], |io| {
        execute_transaction::<M>(
            &transaction_message,
            block_height,
            &block_metadata,
            engine_account_id,
            io,
        )
    });
    let (tx_hash, diff, maybe_result) = result.result;
    let outcome = TransactionIncludedOutcome {
        hash: tx_hash,
        info: transaction_message,
        diff,
        maybe_result,
    };
    Ok(outcome)
}

fn execute_transaction<'db, M: ModExpAlgorithm + 'static>(
    transaction_message: &TransactionMessage,
    block_height: u64,
    block_metadata: &BlockMetadata,
    engine_account_id: AccountId,
    io: EngineStateAccess<'db, 'db, 'db>,
) -> (
    H256,
    Diff,
    Result<Option<TransactionExecutionResult>, error::Error>,
) {
    let signer_account_id = transaction_message.signer.clone();
    let predecessor_account_id = transaction_message.caller.clone();
    let relayer_address =
        aurora_engine_sdk::types::near_account_to_evm_address(predecessor_account_id.as_bytes());
    let near_receipt_id = transaction_message.near_receipt_id;
    let current_account_id = engine_account_id;
    let env = env::Fixed {
        signer_account_id,
        current_account_id,
        predecessor_account_id,
        block_height,
        block_timestamp: block_metadata.timestamp,
        attached_deposit: transaction_message.attached_near,
        random_seed: block_metadata.random_seed,
        prepaid_gas: DEFAULT_PREPAID_GAS,
    };

    let (tx_hash, result) = match &transaction_message.transaction {
        TransactionKind::Submit(tx) => {
            // We can ignore promises in the standalone engine because it processes each receipt separately
            // and it is fed a stream of receipts (it does not schedule them)
            let mut handler = crate::promise::NoScheduler {
                promise_data: &transaction_message.promise_data,
            };
            let tx_data: Vec<u8> = tx.into();
            let tx_hash = aurora_engine_sdk::keccak(&tx_data);
            let args = parameters::SubmitArgs {
                tx_data,
                ..Default::default()
            };
            let result = state::get_state(&io)
                .map(|engine_state| {
                    let submit_result = engine::submit_with_alt_modexp::<_, _, _, M>(
                        io,
                        &env,
                        &args,
                        engine_state,
                        env.current_account_id(),
                        relayer_address,
                        &mut handler,
                    );
                    Some(TransactionExecutionResult::Submit(submit_result))
                })
                .map_err(Into::into);

            (tx_hash, result)
        }
        TransactionKind::SubmitWithArgs(args) => {
            let mut handler = crate::promise::NoScheduler {
                promise_data: &transaction_message.promise_data,
            };
            let tx_hash = aurora_engine_sdk::keccak(&args.tx_data);
            let result = state::get_state(&io)
                .map(|engine_state| {
                    let submit_result = engine::submit_with_alt_modexp::<_, _, _, M>(
                        io,
                        &env,
                        args,
                        engine_state,
                        env.current_account_id(),
                        relayer_address,
                        &mut handler,
                    );
                    Some(TransactionExecutionResult::Submit(submit_result))
                })
                .map_err(Into::into);

            (tx_hash, result)
        }
        other => {
            let result = non_submit_execute::<M>(
                other,
                io,
                env,
                relayer_address,
                &transaction_message.promise_data,
            );
            (near_receipt_id, result)
        }
    };

    let diff = io.get_transaction_diff();

    (tx_hash, diff, result)
}

/// Handles all transaction kinds other than `submit`.
/// The `submit` transaction kind is special because it is the only one where the transaction hash
/// differs from the NEAR receipt hash.
#[allow(clippy::too_many_lines)]
fn non_submit_execute<'db, M: ModExpAlgorithm + 'static>(
    transaction: &TransactionKind,
    mut io: EngineStateAccess<'db, 'db, 'db>,
    env: env::Fixed,
    relayer_address: Address,
    promise_data: &[Option<Vec<u8>>],
) -> Result<Option<TransactionExecutionResult>, error::Error> {
    let is_disabled_legacy_nep141 =
        aurora_engine::connector::EthConnectorContract::init_instance(io)?
            .is_disabled_legacy_nep141();
    let result = match transaction {
        TransactionKind::Call(args) => {
            // We can ignore promises in the standalone engine (see above)
            let mut handler = crate::promise::NoScheduler { promise_data };
            let mut engine: engine::Engine<_, _, M> =
                engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;

            let result = engine.call_with_args(args.clone(), &mut handler);

            Some(TransactionExecutionResult::Submit(result))
        }

        TransactionKind::Deploy(input) => {
            // We can ignore promises in the standalone engine (see above)
            let mut handler = crate::promise::NoScheduler { promise_data };
            let mut engine: engine::Engine<_, _, M> =
                engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;

            let result = engine.deploy_code_with_input(input.clone(), &mut handler);

            Some(TransactionExecutionResult::Submit(result))
        }

        TransactionKind::DeployErc20(args) => {
            // No promises can be created by `deploy_erc20_token`
            let mut handler = crate::promise::NoScheduler { promise_data };
            let result = engine::deploy_erc20_token(args.clone(), io, &env, &mut handler)?;

            Some(TransactionExecutionResult::DeployErc20(result))
        }

        TransactionKind::FtOnTransfer(args) => {
            // No promises can be created by `ft_on_transfer`
            let mut handler = crate::promise::NoScheduler { promise_data };
            let mut engine: engine::Engine<_, _, M> =
                engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;

            if env.predecessor_account_id == env.current_account_id {
                legacy_connector::EthConnectorContract::init_instance(io)?
                    .ft_on_transfer(&engine, args)?;
            } else {
                engine.receive_erc20_tokens(
                    &env.predecessor_account_id,
                    args,
                    &env.current_account_id,
                    &mut handler,
                );
            }

            None
        }

        TransactionKind::FtTransferCall(_) if is_disabled_legacy_nep141 => None,
        TransactionKind::FtTransferCall(args) => {
            let mut connector = legacy_connector::EthConnectorContract::init_instance(io)?;
            let promise_args = connector.ft_transfer_call(
                env.predecessor_account_id.clone(),
                env.current_account_id.clone(),
                args.clone(),
                env.prepaid_gas,
            )?;

            Some(TransactionExecutionResult::Promise(promise_args))
        }

        TransactionKind::ResolveTransfer(_, _) if is_disabled_legacy_nep141 => None,
        TransactionKind::ResolveTransfer(args, promise_result) => {
            let mut connector = legacy_connector::EthConnectorContract::init_instance(io)?;
            connector.ft_resolve_transfer(args, promise_result.clone());

            None
        }

        TransactionKind::FtTransfer(_) if is_disabled_legacy_nep141 => None,
        TransactionKind::FtTransfer(args) => {
            let mut connector = legacy_connector::EthConnectorContract::init_instance(io)?;
            connector.ft_transfer(&env.predecessor_account_id, args)?;

            None
        }

        TransactionKind::Withdraw(_) if is_disabled_legacy_nep141 => None,
        TransactionKind::Withdraw(args) => {
            let mut connector = legacy_connector::EthConnectorContract::init_instance(io)?;
            connector.withdraw_eth_from_near(
                &env.current_account_id,
                &env.predecessor_account_id,
                args,
            )?;

            None
        }

        TransactionKind::Deposit(_) if is_disabled_legacy_nep141 => None,
        TransactionKind::Deposit(raw_proof) => {
            let connector_contract = legacy_connector::EthConnectorContract::init_instance(io)?;
            let promise_args = connector_contract.deposit(
                raw_proof.clone(),
                env.current_account_id(),
                env.predecessor_account_id(),
            )?;

            Some(TransactionExecutionResult::Promise(promise_args))
        }

        TransactionKind::FinishDeposit(_) if is_disabled_legacy_nep141 => None,
        TransactionKind::FinishDeposit(finish_args) => {
            let mut connector = legacy_connector::EthConnectorContract::init_instance(io)?;
            let maybe_promise_args = connector.finish_deposit(
                env.predecessor_account_id(),
                env.current_account_id(),
                finish_args.clone(),
                env.prepaid_gas,
            )?;

            maybe_promise_args.map(TransactionExecutionResult::Promise)
        }

        TransactionKind::StorageDeposit(_) if is_disabled_legacy_nep141 => None,
        TransactionKind::StorageDeposit(args) => {
            let mut connector = legacy_connector::EthConnectorContract::init_instance(io)?;
            let _promise = connector.storage_deposit(
                env.predecessor_account_id,
                Yocto::new(env.attached_deposit),
                args.clone(),
            )?;

            None
        }

        TransactionKind::StorageUnregister(_) if is_disabled_legacy_nep141 => None,
        TransactionKind::StorageUnregister(force) => {
            let mut connector = legacy_connector::EthConnectorContract::init_instance(io)?;
            let _promise = connector.storage_unregister(env.predecessor_account_id, *force)?;

            None
        }

        TransactionKind::StorageWithdraw(_) if is_disabled_legacy_nep141 => None,
        TransactionKind::StorageWithdraw(args) => {
            let mut connector = legacy_connector::EthConnectorContract::init_instance(io)?;
            connector.storage_withdraw(&env.predecessor_account_id, args)?;

            None
        }

        TransactionKind::SetPausedFlags(_) if is_disabled_legacy_nep141 => None,
        TransactionKind::SetPausedFlags(args) => {
            let mut connector = legacy_connector::EthConnectorContract::init_instance(io)?;
            connector.set_paused_flags(args);

            None
        }

        TransactionKind::RegisterRelayer(evm_address) => {
            let mut engine: engine::Engine<_, _, M> =
                engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;
            engine.register_relayer(env.predecessor_account_id.as_bytes(), *evm_address);

            None
        }

        TransactionKind::RefundOnError(maybe_args) => {
            let result: Result<Option<TransactionExecutionResult>, state::EngineStateError> =
                maybe_args
                    .clone()
                    .map(|args| {
                        let mut handler = crate::promise::NoScheduler { promise_data };
                        let engine_state = state::get_state(&io)?;
                        let result =
                            engine::refund_on_error(io, &env, engine_state, &args, &mut handler);
                        Ok(TransactionExecutionResult::Submit(result))
                    })
                    .transpose();

            result?
        }

        TransactionKind::SetConnectorData(_) if is_disabled_legacy_nep141 => None,
        TransactionKind::SetConnectorData(args) => {
            let mut connector_io = io;
            legacy_connector::set_contract_data(&mut connector_io, args.clone())?;

            None
        }

        TransactionKind::NewConnector(_) if is_disabled_legacy_nep141 => None,
        TransactionKind::NewConnector(args) => {
            legacy_connector::EthConnectorContract::create_contract(
                io,
                env.current_account_id,
                args.clone(),
            )?;

            None
        }

        TransactionKind::SetEthConnectorContractAccount(args) => {
            use aurora_engine::admin_controlled::AdminControlled;

            let mut connector = aurora_engine::connector::EthConnectorContract::init_instance(io)?;
            connector.set_eth_connector_contract_account(&args.account);

            None
        }

        TransactionKind::DisableLegacyNEP141 => {
            let mut connector = aurora_engine::connector::EthConnectorContract::init_instance(io)?;
            connector.disable_legacy_nep141();

            None
        }

        TransactionKind::NewEngine(args) => {
            state::set_state(&mut io, &args.clone().into())?;

            None
        }
        TransactionKind::FactoryUpdate(bytecode) => {
            let router_bytecode = xcc::RouterCode::borrowed(bytecode);
            xcc::update_router_code(&mut io, &router_bytecode);

            None
        }
        TransactionKind::FactoryUpdateAddressVersion(args) => {
            xcc::set_code_version_of_address(&mut io, &args.address, args.version);

            None
        }
        TransactionKind::FactorySetWNearAddress(address) => {
            xcc::set_wnear_address(&mut io, address);

            None
        }
        TransactionKind::FundXccSubAccount(args) => {
            let mut handler = crate::promise::NoScheduler { promise_data };
            xcc::fund_xcc_sub_account(&io, &mut handler, &env, args.clone())?;

            None
        }
        TransactionKind::Unknown => None,
        // Not handled in this function; is handled by the general `execute_transaction` function
        TransactionKind::Submit(_) | TransactionKind::SubmitWithArgs(_) => unreachable!(),
        TransactionKind::PausePrecompiles(args) => {
            let precompiles_to_pause = PrecompileFlags::from_bits_truncate(args.paused_mask);

            let mut pauser = EnginePrecompilesPauser::from_io(io);
            pauser.pause_precompiles(precompiles_to_pause);

            None
        }
        TransactionKind::ResumePrecompiles(args) => {
            let precompiles_to_resume = PrecompileFlags::from_bits_truncate(args.paused_mask);

            let mut pauser = EnginePrecompilesPauser::from_io(io);
            pauser.resume_precompiles(precompiles_to_resume);

            None
        }
        TransactionKind::SetOwner(args) => {
            let mut prev = state::get_state(&io)?;

            prev.owner_id = args.clone().new_owner;
            state::set_state(&mut io, &prev)?;

            None
        }
        TransactionKind::SetUpgradeDelayBlocks(args) => {
            let mut prev = state::get_state(&io)?;

            prev.upgrade_delay_blocks = args.upgrade_delay_blocks;
            state::set_state(&mut io, &prev)?;

            None
        }
        TransactionKind::PauseContract => {
            let mut prev = state::get_state(&io)?;

            prev.is_paused = true;
            state::set_state(&mut io, &prev)?;

            None
        }
        TransactionKind::ResumeContract => {
            let mut prev = state::get_state(&io)?;

            prev.is_paused = false;
            state::set_state(&mut io, &prev)?;

            None
        }
        TransactionKind::SetKeyManager(args) => {
            let mut prev = state::get_state(&io)?;

            prev.key_manager = args.key_manager.clone();
            state::set_state(&mut io, &prev)?;

            None
        }
        TransactionKind::AddRelayerKey(args) => {
            engine::add_function_call_key(&mut io, &args.public_key);

            None
        }
        TransactionKind::RemoveRelayerKey(args) => {
            engine::remove_function_call_key(&mut io, &args.public_key)?;

            None
        }
        TransactionKind::SetFixedGasCost(args) => {
            silo::set_fixed_gas_cost(&mut io, args.cost);
            None
        }
        TransactionKind::SetSiloParams(args) => {
            silo::set_silo_params(&mut io, args.clone());
            None
        }
        TransactionKind::AddEntryToWhitelist(args) => {
            silo::add_entry_to_whitelist(&io, args);
            None
        }
        TransactionKind::AddEntryToWhitelistBatch(args) => {
            silo::add_entry_to_whitelist_batch(&io, args.clone());
            None
        }
        TransactionKind::RemoveEntryFromWhitelist(args) => {
            silo::remove_entry_from_whitelist(&io, args);
            None
        }
        TransactionKind::SetWhitelistStatus(args) => {
            silo::set_whitelist_status(&io, args);
            None
        }
    };

    Ok(result)
}

#[derive(Debug)]
pub enum ConsumeMessageOutcome {
    BlockAdded,
    FailedTransactionIgnored,
    TransactionIncluded(Box<TransactionIncludedOutcome>),
}

#[derive(Debug)]
pub struct TransactionIncludedOutcome {
    pub hash: aurora_engine_types::H256,
    pub info: TransactionMessage,
    pub diff: crate::Diff,
    pub maybe_result: Result<Option<TransactionExecutionResult>, error::Error>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionExecutionResult {
    Submit(engine::EngineResult<SubmitResult>),
    DeployErc20(Address),
    Promise(PromiseWithCallbackArgs),
}

pub mod error {
    use aurora_engine::{engine, state, xcc};
    use aurora_engine_standalone_nep141_legacy::{fungible_token, legacy_connector};

    #[derive(Debug)]
    pub enum Error {
        EngineState(state::EngineStateError),
        Engine(engine::EngineError),
        DeployErc20(engine::DeployErc20Error),
        FtOnTransfer(legacy_connector::error::FtTransferCallError),
        Deposit(legacy_connector::error::DepositError),
        FinishDeposit(legacy_connector::error::FinishDepositError),
        FtTransfer(fungible_token::error::TransferError),
        FtWithdraw(legacy_connector::error::WithdrawError),
        FtStorageFunding(fungible_token::error::StorageFundingError),
        InvalidAddress(aurora_engine_types::types::address::error::AddressError),
        ConnectorInit(legacy_connector::error::InitContractError),
        LegacyConnectorStorage(legacy_connector::error::StorageReadError),
        ConnectorStorage(aurora_engine::connector::error::StorageReadError),
        FundXccError(xcc::FundXccError),
    }

    impl From<state::EngineStateError> for Error {
        fn from(e: state::EngineStateError) -> Self {
            Self::EngineState(e)
        }
    }

    impl From<engine::EngineError> for Error {
        fn from(e: engine::EngineError) -> Self {
            Self::Engine(e)
        }
    }

    impl From<engine::DeployErc20Error> for Error {
        fn from(e: engine::DeployErc20Error) -> Self {
            Self::DeployErc20(e)
        }
    }

    impl From<legacy_connector::error::FtTransferCallError> for Error {
        fn from(e: legacy_connector::error::FtTransferCallError) -> Self {
            Self::FtOnTransfer(e)
        }
    }

    impl From<legacy_connector::error::DepositError> for Error {
        fn from(e: legacy_connector::error::DepositError) -> Self {
            Self::Deposit(e)
        }
    }

    impl From<legacy_connector::error::FinishDepositError> for Error {
        fn from(e: legacy_connector::error::FinishDepositError) -> Self {
            Self::FinishDeposit(e)
        }
    }

    impl From<fungible_token::error::TransferError> for Error {
        fn from(e: fungible_token::error::TransferError) -> Self {
            Self::FtTransfer(e)
        }
    }

    impl From<legacy_connector::error::WithdrawError> for Error {
        fn from(e: legacy_connector::error::WithdrawError) -> Self {
            Self::FtWithdraw(e)
        }
    }

    impl From<fungible_token::error::StorageFundingError> for Error {
        fn from(e: fungible_token::error::StorageFundingError) -> Self {
            Self::FtStorageFunding(e)
        }
    }

    impl From<aurora_engine_types::types::address::error::AddressError> for Error {
        fn from(e: aurora_engine_types::types::address::error::AddressError) -> Self {
            Self::InvalidAddress(e)
        }
    }

    impl From<legacy_connector::error::InitContractError> for Error {
        fn from(e: legacy_connector::error::InitContractError) -> Self {
            Self::ConnectorInit(e)
        }
    }

    impl From<legacy_connector::error::StorageReadError> for Error {
        fn from(e: legacy_connector::error::StorageReadError) -> Self {
            Self::LegacyConnectorStorage(e)
        }
    }

    impl From<aurora_engine::connector::error::StorageReadError> for Error {
        fn from(e: aurora_engine::connector::error::StorageReadError) -> Self {
            Self::ConnectorStorage(e)
        }
    }

    impl From<xcc::FundXccError> for Error {
        fn from(e: xcc::FundXccError) -> Self {
            Self::FundXccError(e)
        }
    }
}

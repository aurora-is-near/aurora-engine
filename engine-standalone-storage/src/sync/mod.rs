use crate::engine_state::EngineStateAccess;
use aurora_engine::contract_methods::silo;
use aurora_engine::{
    contract_methods, engine,
    parameters::{self, SubmitResult},
};
use aurora_engine_modexp::ModExpAlgorithm;
use aurora_engine_sdk::{
    env::{self, DEFAULT_PREPAID_GAS},
    io::IO,
};
use aurora_engine_transactions::EthTransactionKind;
use aurora_engine_types::types::NearGas;
use aurora_engine_types::{
    account_id::AccountId,
    borsh::BorshDeserialize,
    parameters::{silo as silo_params, xcc, PromiseWithCallbackArgs},
    types::Address,
    H256,
};
use std::{io, str::FromStr};

pub mod types;

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
            let deploy_args = parameters::DeployErc20TokenArgs::deserialize(&bytes).map_err(f)?;
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
        TransactionKindTag::ExitToNear => {
            if promise_data.first().and_then(Option::as_ref).is_none() {
                TransactionKind::ExitToNear(None)
            } else {
                let args = aurora_engine_types::parameters::ExitToNearPrecompileCallbackCallArgs::try_from_slice(&bytes)
                             .map_err(f)?;
                TransactionKind::ExitToNear(Some(args))
            }
        }
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
        TransactionKindTag::WithdrawWnearToRouter => {
            let args = xcc::WithdrawWnearToRouterArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::WithdrawWnearToRouter(args)
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
            let args: parameters::RelayerKeyManagerArgs = serde_json::from_slice(bytes.as_slice())
                .map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;
            TransactionKind::SetKeyManager(args)
        }
        TransactionKindTag::AddRelayerKey => {
            let args = parameters::RelayerKeyArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::AddRelayerKey(args)
        }
        TransactionKindTag::StoreRelayerKeyCallback => {
            let args = parameters::RelayerKeyArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::StoreRelayerKeyCallback(args)
        }
        TransactionKindTag::RemoveRelayerKey => {
            let args = parameters::RelayerKeyArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::RemoveRelayerKey(args)
        }
        TransactionKindTag::StartHashchain => {
            let args = parameters::StartHashchainArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::StartHashchain(args)
        }
        TransactionKindTag::SetErc20Metadata => {
            let args: parameters::SetErc20MetadataArgs =
                serde_json::from_slice(&bytes).map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;
            TransactionKind::SetErc20Metadata(args)
        }
        TransactionKindTag::SetFixedGas => {
            let args = silo_params::FixedGasArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetFixedGas(args)
        }
        TransactionKindTag::SetErc20FallbackAddress => {
            let args = silo_params::Erc20FallbackAddressArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetErc20FallbackAddress(args)
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
        TransactionKindTag::SetWhitelistsStatuses => {
            let args: Vec<silo_params::WhitelistStatusArgs> =
                BorshDeserialize::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetWhitelistsStatuses(args)
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
        TransactionKindTag::MirrorErc20TokenCallback => {
            let args = parameters::MirrorErc20TokenArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::MirrorErc20TokenCallback(args)
        }
        TransactionKindTag::Unknown => {
            return Err(ParseTransactionKindError::UnknownMethodName {
                name: method_name.into(),
            });
        }
    };
    Ok(tx_kind)
}

/// Note: this function does not automatically commit transaction messages to the storage.
/// If you want the transaction diff committed then you must call the `commit` method on
/// the outcome of this function.
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
                .with_engine_access(
                    block_height,
                    transaction_position,
                    &transaction_message.raw_input,
                    |io| {
                        execute_transaction::<_, M, _>(
                            transaction_message.as_ref(),
                            block_height,
                            &block_metadata,
                            engine_account_id,
                            io,
                            EngineStateAccess::get_transaction_diff,
                        )
                    },
                )
                .result;
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
    let result = storage.with_engine_access(
        block_height,
        transaction_position,
        &transaction_message.raw_input,
        |io| {
            execute_transaction::<_, M, _>(
                &transaction_message,
                block_height,
                &block_metadata,
                engine_account_id,
                io,
                EngineStateAccess::get_transaction_diff,
            )
        },
    );
    let (tx_hash, diff, maybe_result) = result.result;
    let outcome = TransactionIncludedOutcome {
        hash: tx_hash,
        info: transaction_message,
        diff,
        maybe_result,
    };
    Ok(outcome)
}

pub fn execute_transaction<I, M, F>(
    transaction_message: &TransactionMessage,
    block_height: u64,
    block_metadata: &BlockMetadata,
    engine_account_id: AccountId,
    io: I,
    get_diff: F,
) -> (
    H256,
    Diff,
    Result<Option<TransactionExecutionResult>, error::Error>,
)
where
    I: IO + Copy,
    M: ModExpAlgorithm + 'static,
    F: FnOnce(&I) -> Diff,
{
    let signer_account_id = transaction_message.signer.clone();
    let predecessor_account_id = transaction_message.caller.clone();
    let near_receipt_id = transaction_message.near_receipt_id;
    let current_account_id = engine_account_id;
    let random_seed = compute_random_seed(
        &transaction_message.action_hash,
        &block_metadata.random_seed,
    );
    let env = env::Fixed {
        signer_account_id,
        current_account_id,
        predecessor_account_id,
        block_height,
        block_timestamp: block_metadata.timestamp,
        attached_deposit: transaction_message.attached_near,
        random_seed,
        prepaid_gas: DEFAULT_PREPAID_GAS,
        used_gas: NearGas::new(0),
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
            let result = contract_methods::evm_transactions::submit(io, &env, &mut handler)
                .map(|submit_result| Some(TransactionExecutionResult::Submit(Ok(submit_result))))
                .map_err(Into::into);

            (tx_hash, result)
        }
        TransactionKind::SubmitWithArgs(args) => {
            let mut handler = crate::promise::NoScheduler {
                promise_data: &transaction_message.promise_data,
            };
            let tx_hash = aurora_engine_sdk::keccak(&args.tx_data);
            let result =
                contract_methods::evm_transactions::submit_with_args(io, &env, &mut handler)
                    .map(|submit_result| {
                        Some(TransactionExecutionResult::Submit(Ok(submit_result)))
                    })
                    .map_err(Into::into);

            (tx_hash, result)
        }
        other => {
            let result = non_submit_execute(other, io, &env, &transaction_message.promise_data);
            (near_receipt_id, result)
        }
    };

    let diff = get_diff(&io);

    (tx_hash, diff, result)
}

/// Based on nearcore implementation:
/// <https://github.com/near/nearcore/blob/00ca2f3f73e2a547ba881f76ecc59450dbbef6e2/core/primitives/src/utils.rs#L295>
fn compute_random_seed(action_hash: &H256, block_random_value: &H256) -> H256 {
    const BYTES_LEN: usize = 32 + 32;
    let mut bytes: Vec<u8> = Vec::with_capacity(BYTES_LEN);
    bytes.extend_from_slice(action_hash.as_bytes());
    bytes.extend_from_slice(block_random_value.as_bytes());
    aurora_engine_sdk::sha256(&bytes)
}

/// Handles all transaction kinds other than `submit`.
/// The `submit` transaction kind is special because it is the only one where the transaction hash
/// differs from the NEAR receipt hash.
#[allow(
    clippy::too_many_lines,
    clippy::match_same_arms,
    clippy::cognitive_complexity
)]
fn non_submit_execute<I: IO + Copy>(
    transaction: &TransactionKind,
    mut io: I,
    env: &env::Fixed,
    promise_data: &[Option<Vec<u8>>],
) -> Result<Option<TransactionExecutionResult>, error::Error> {
    let result = match transaction {
        TransactionKind::Call(_) => {
            // We can ignore promises in the standalone engine (see above)
            let mut handler = crate::promise::NoScheduler { promise_data };
            let result = contract_methods::evm_transactions::call(io, env, &mut handler)?;

            Some(TransactionExecutionResult::Submit(Ok(result)))
        }

        TransactionKind::Deploy(_) => {
            // We can ignore promises in the standalone engine (see above)
            let mut handler = crate::promise::NoScheduler { promise_data };
            let result = contract_methods::evm_transactions::deploy_code(io, env, &mut handler)?;

            Some(TransactionExecutionResult::Submit(Ok(result)))
        }
        TransactionKind::DeployErc20(_) => {
            // No promises can be created by `deploy_erc20_token`
            let mut handler = crate::promise::NoScheduler { promise_data };
            let result = contract_methods::connector::deploy_erc20_token(io, env, &mut handler)?;

            Some(TransactionExecutionResult::DeployErc20(result))
        }
        TransactionKind::FtOnTransfer(_) => {
            // No promises can be created by `ft_on_transfer`
            let mut handler = crate::promise::NoScheduler { promise_data };
            let maybe_output = contract_methods::connector::ft_on_transfer(io, env, &mut handler)?;

            maybe_output.map(|result| TransactionExecutionResult::Submit(Ok(result)))
        }
        TransactionKind::FtTransferCall(_) => {
            #[cfg(feature = "ext-connector")]
            return Ok(None);

            #[cfg(not(feature = "ext-connector"))]
            {
                let mut handler = crate::promise::NoScheduler { promise_data };
                let maybe_promise_args =
                    contract_methods::connector::ft_transfer_call(io, env, &mut handler)?;

                maybe_promise_args.map(TransactionExecutionResult::Promise)
            }
        }
        TransactionKind::ResolveTransfer(_, _) => {
            #[cfg(not(feature = "ext-connector"))]
            {
                let handler = crate::promise::NoScheduler { promise_data };
                contract_methods::connector::ft_resolve_transfer(io, env, &handler)?;
            }

            None
        }
        TransactionKind::FtTransfer(_) => {
            #[cfg(not(feature = "ext-connector"))]
            contract_methods::connector::ft_transfer(io, env)?;

            None
        }
        TransactionKind::Withdraw(_) => {
            #[cfg(not(feature = "ext-connector"))]
            contract_methods::connector::withdraw(io, env)?;

            None
        }
        TransactionKind::Deposit(_) => {
            #[cfg(feature = "ext-connector")]
            return Ok(None);

            #[cfg(not(feature = "ext-connector"))]
            {
                let mut handler = crate::promise::NoScheduler { promise_data };
                let maybe_promise_args =
                    contract_methods::connector::deposit(io, env, &mut handler)?;
                maybe_promise_args.map(TransactionExecutionResult::Promise)
            }
        }

        TransactionKind::FinishDeposit(_) => {
            #[cfg(feature = "ext-connector")]
            return Ok(None);

            #[cfg(not(feature = "ext-connector"))]
            {
                let mut handler = crate::promise::NoScheduler { promise_data };
                let maybe_promise_args =
                    contract_methods::connector::finish_deposit(io, env, &mut handler)?;

                maybe_promise_args.map(TransactionExecutionResult::Promise)
            }
        }

        TransactionKind::StorageDeposit(_) => {
            #[cfg(not(feature = "ext-connector"))]
            {
                let mut handler = crate::promise::NoScheduler { promise_data };
                contract_methods::connector::storage_deposit(io, env, &mut handler)?;
            }

            None
        }
        TransactionKind::StorageUnregister(_) => {
            #[cfg(not(feature = "ext-connector"))]
            {
                let mut handler = crate::promise::NoScheduler { promise_data };
                contract_methods::connector::storage_unregister(io, env, &mut handler)?;
            }

            None
        }
        TransactionKind::StorageWithdraw(_) => {
            #[cfg(not(feature = "ext-connector"))]
            contract_methods::connector::storage_withdraw(io, env)?;

            None
        }
        TransactionKind::SetPausedFlags(_) => {
            #[cfg(not(feature = "ext-connector"))]
            contract_methods::connector::set_paused_flags(io, env)?;

            None
        }
        TransactionKind::RegisterRelayer(_) => {
            contract_methods::admin::register_relayer(io, env)?;

            None
        }
        TransactionKind::ExitToNear(_) => {
            let mut handler = crate::promise::NoScheduler { promise_data };
            let maybe_result = contract_methods::connector::exit_to_near_precompile_callback(
                io,
                env,
                &mut handler,
            )?;

            maybe_result.map(|submit_result| TransactionExecutionResult::Submit(Ok(submit_result)))
        }
        TransactionKind::SetConnectorData(_) => {
            #[cfg(not(feature = "ext-connector"))]
            contract_methods::connector::set_eth_connector_contract_data(io, env)?;

            None
        }
        TransactionKind::NewConnector(_) => {
            #[cfg(not(feature = "ext-connector"))]
            contract_methods::connector::new_eth_connector(io, env)?;

            None
        }
        TransactionKind::NewEngine(_) => {
            contract_methods::admin::new(io, env)?;

            None
        }
        TransactionKind::SetEthConnectorContractAccount(_) => {
            #[cfg(feature = "ext-connector")]
            contract_methods::connector::set_eth_connector_contract_account(io, env)?;

            None
        }
        TransactionKind::FactoryUpdate(_) => {
            contract_methods::xcc::factory_update(io, env)?;

            None
        }
        TransactionKind::FactoryUpdateAddressVersion(_) => {
            let handler = crate::promise::NoScheduler { promise_data };
            contract_methods::xcc::factory_update_address_version(io, env, &handler)?;

            None
        }
        TransactionKind::FactorySetWNearAddress(_) => {
            contract_methods::xcc::factory_set_wnear_address(io, env)?;

            None
        }
        TransactionKind::FundXccSubAccount(_) => {
            let mut handler = crate::promise::NoScheduler { promise_data };
            contract_methods::xcc::fund_xcc_sub_account(io, env, &mut handler)?;

            None
        }
        TransactionKind::WithdrawWnearToRouter(_) => {
            let mut handler = crate::promise::NoScheduler { promise_data };
            let result = contract_methods::xcc::withdraw_wnear_to_router(io, env, &mut handler)?;

            Some(TransactionExecutionResult::Submit(Ok(result)))
        }
        TransactionKind::Unknown => None,
        // Not handled in this function; is handled by the general `execute_transaction` function
        TransactionKind::Submit(_) | TransactionKind::SubmitWithArgs(_) => unreachable!(),
        TransactionKind::PausePrecompiles(_) => {
            contract_methods::admin::pause_precompiles(io, env)?;

            None
        }
        TransactionKind::ResumePrecompiles(_) => {
            contract_methods::admin::resume_precompiles(io, env)?;

            None
        }
        TransactionKind::SetOwner(_) => {
            contract_methods::admin::set_owner(io, env)?;

            None
        }
        TransactionKind::SetUpgradeDelayBlocks(_) => {
            contract_methods::admin::set_upgrade_delay_blocks(io, env)?;

            None
        }
        TransactionKind::PauseContract => {
            contract_methods::admin::pause_contract(io, env)?;

            None
        }
        TransactionKind::ResumeContract => {
            contract_methods::admin::resume_contract(io, env)?;

            None
        }
        TransactionKind::SetKeyManager(_) => {
            contract_methods::admin::set_key_manager(io, env)?;

            None
        }
        TransactionKind::AddRelayerKey(_) => {
            let mut handler = crate::promise::NoScheduler { promise_data };
            contract_methods::admin::add_relayer_key(io, env, &mut handler)?;

            None
        }
        TransactionKind::StoreRelayerKeyCallback(_) => {
            contract_methods::admin::store_relayer_key_callback(io, env)?;

            None
        }
        TransactionKind::RemoveRelayerKey(_) => {
            let mut handler = crate::promise::NoScheduler { promise_data };
            contract_methods::admin::remove_relayer_key(io, env, &mut handler)?;

            None
        }
        TransactionKind::StartHashchain(_) => {
            contract_methods::admin::start_hashchain(io, env)?;

            None
        }
        TransactionKind::SetErc20Metadata(_) => {
            let mut handler = crate::promise::NoScheduler { promise_data };
            contract_methods::connector::set_erc20_metadata(io, env, &mut handler)?;

            None
        }
        TransactionKind::SetFixedGas(args) => {
            silo::set_fixed_gas(&mut io, args.fixed_gas);
            None
        }
        TransactionKind::SetErc20FallbackAddress(args) => {
            silo::set_erc20_fallback_address(&mut io, args.address);
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
        TransactionKind::SetWhitelistsStatuses(args) => {
            silo::set_whitelists_statuses(&io, args.clone());
            None
        }
        TransactionKind::MirrorErc20TokenCallback(_) => {
            let mut handler = crate::promise::NoScheduler { promise_data };
            contract_methods::connector::mirror_erc20_token_callback(io, env, &mut handler)?;

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

impl ConsumeMessageOutcome {
    pub fn commit(&self, storage: &mut Storage) -> Result<(), crate::error::Error> {
        if let Self::TransactionIncluded(x) = self {
            x.commit(storage)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct TransactionIncludedOutcome {
    pub hash: H256,
    pub info: TransactionMessage,
    pub diff: Diff,
    pub maybe_result: Result<Option<TransactionExecutionResult>, error::Error>,
}

impl TransactionIncludedOutcome {
    pub fn commit(&self, storage: &mut Storage) -> Result<(), crate::error::Error> {
        match self.maybe_result.as_ref() {
            Err(_) | Ok(Some(TransactionExecutionResult::Submit(Err(_)))) => (), // do not persist if Engine encounters an error
            _ => storage.set_transaction_included(self.hash, &self.info, &self.diff)?,
        };
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionExecutionResult {
    Submit(engine::EngineResult<SubmitResult>),
    DeployErc20(Address),
    Promise(PromiseWithCallbackArgs),
}

pub mod error {
    use aurora_engine::contract_methods::connector::errors;
    use aurora_engine::{contract_methods, engine, state, xcc};

    #[derive(Debug)]
    pub enum Error {
        EngineState(state::EngineStateError),
        Engine(engine::EngineError),
        DeployErc20(engine::DeployErc20Error),
        FtOnTransfer(errors::FtTransferCallError),
        Deposit(errors::DepositError),
        FinishDeposit(errors::FinishDepositError),
        FtTransfer(errors::TransferError),
        FtWithdraw(errors::WithdrawError),
        FtStorageFunding(errors::StorageFundingError),
        InvalidAddress(aurora_engine_types::types::address::error::AddressError),
        ConnectorInit(errors::InitContractError),
        ConnectorStorage(errors::StorageReadError),
        FundXccError(xcc::FundXccError),
        ContractError(contract_methods::ContractError),
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

    impl From<errors::FtTransferCallError> for Error {
        fn from(e: errors::FtTransferCallError) -> Self {
            Self::FtOnTransfer(e)
        }
    }

    impl From<errors::DepositError> for Error {
        fn from(e: errors::DepositError) -> Self {
            Self::Deposit(e)
        }
    }

    impl From<errors::FinishDepositError> for Error {
        fn from(e: errors::FinishDepositError) -> Self {
            Self::FinishDeposit(e)
        }
    }

    impl From<errors::TransferError> for Error {
        fn from(e: errors::TransferError) -> Self {
            Self::FtTransfer(e)
        }
    }

    impl From<errors::WithdrawError> for Error {
        fn from(e: errors::WithdrawError) -> Self {
            Self::FtWithdraw(e)
        }
    }

    impl From<errors::StorageFundingError> for Error {
        fn from(e: errors::StorageFundingError) -> Self {
            Self::FtStorageFunding(e)
        }
    }

    impl From<aurora_engine_types::types::address::error::AddressError> for Error {
        fn from(e: aurora_engine_types::types::address::error::AddressError) -> Self {
            Self::InvalidAddress(e)
        }
    }

    impl From<errors::InitContractError> for Error {
        fn from(e: errors::InitContractError) -> Self {
            Self::ConnectorInit(e)
        }
    }

    impl From<errors::StorageReadError> for Error {
        fn from(e: errors::StorageReadError) -> Self {
            Self::ConnectorStorage(e)
        }
    }

    impl From<xcc::FundXccError> for Error {
        fn from(e: xcc::FundXccError) -> Self {
            Self::FundXccError(e)
        }
    }

    impl From<contract_methods::ContractError> for Error {
        fn from(e: contract_methods::ContractError) -> Self {
            Self::ContractError(e)
        }
    }
}

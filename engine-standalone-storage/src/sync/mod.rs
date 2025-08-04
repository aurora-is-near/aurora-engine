use std::fmt::Debug;
use std::mem;
use std::sync::Arc;
use std::{io, str::FromStr};

use aurora_engine::engine::EngineError;
use aurora_engine::parameters::SubmitResult;
use aurora_engine_modexp::ModExpAlgorithm;
use aurora_engine_sdk::{
    env::{self, DEFAULT_PREPAID_GAS},
    io::IO,
};
use aurora_engine_transactions::EthTransactionKind;
use aurora_engine_types::borsh;
use aurora_engine_types::parameters::{connector, engine};
use aurora_engine_types::types::NearGas;
use aurora_engine_types::{
    account_id::AccountId,
    borsh::BorshDeserialize,
    parameters::{silo as silo_params, xcc, PromiseWithCallbackArgs},
    types::Address,
    H256,
};
use engine_standalone_tracing::types::call_tracer::CallTracer;
use engine_standalone_tracing::{Logs, TraceLog};
use near_vm_runner::logic::errors::VMRunnerError;
use near_vm_runner::logic::types::PromiseResult;
use thiserror::Error;

pub mod types;

use crate::engine_state::EngineStateAccess;
use crate::runner::{Context, ContractRunner};
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
            let args = engine::SubmitArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SubmitWithArgs(args)
        }
        TransactionKindTag::Call => {
            let call_args = engine::CallArgs::deserialize(&bytes).ok_or_else(|| {
                ParseTransactionKindError::failed_deserialization::<io::Error>(tx_kind_tag, None)
            })?;
            TransactionKind::Call(call_args)
        }
        TransactionKindTag::PausePrecompiles => {
            let args = engine::PausePrecompilesCallArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::PausePrecompiles(args)
        }
        TransactionKindTag::ResumePrecompiles => {
            let args = engine::PausePrecompilesCallArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::ResumePrecompiles(args)
        }
        TransactionKindTag::SetOwner => {
            let args = engine::SetOwnerArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetOwner(args)
        }
        TransactionKindTag::Deploy => TransactionKind::Deploy(bytes),
        TransactionKindTag::DeployErc20 => {
            let deploy_args = engine::DeployErc20TokenArgs::deserialize(&bytes).map_err(f)?;
            TransactionKind::DeployErc20(deploy_args)
        }
        TransactionKindTag::DeployErc20Callback => {
            let args = AccountId::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::DeployErc20Callback(args)
        }
        TransactionKindTag::FtOnTransfer => {
            let transfer_args: connector::FtOnTransferArgs =
                serde_json::from_slice(bytes.as_slice()).map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;

            TransactionKind::FtOnTransfer(transfer_args)
        }
        TransactionKindTag::Deposit => TransactionKind::Deposit(bytes),
        TransactionKindTag::FtTransferCall => {
            let transfer_args: connector::FtTransferCallArgs =
                serde_json::from_slice(bytes.as_slice()).map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;

            TransactionKind::FtTransferCall(transfer_args)
        }
        TransactionKindTag::FinishDeposit => {
            let args = connector::FinishDepositArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::FinishDeposit(args)
        }
        TransactionKindTag::ResolveTransfer => {
            let args = connector::FtResolveTransferArgs::try_from_slice(&bytes).map_err(f)?;
            let promise_result = promise_data
                .first()
                .and_then(Option::as_ref)
                .map_or(aurora_engine_types::types::PromiseResult::Failed, |bytes| {
                    aurora_engine_types::types::PromiseResult::Successful(bytes.clone())
                });
            TransactionKind::ResolveTransfer(args, promise_result)
        }
        TransactionKindTag::FtTransfer => {
            let args: connector::FtTransferArgs = serde_json::from_slice(bytes.as_slice())
                .map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;

            TransactionKind::FtTransfer(args)
        }
        TransactionKindTag::Withdraw => {
            let args = connector::WithdrawCallArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::Withdraw(args)
        }
        TransactionKindTag::StorageDeposit => {
            let args: connector::StorageDepositArgs = serde_json::from_slice(bytes.as_slice())
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
            let args: connector::StorageWithdrawArgs = serde_json::from_slice(bytes.as_slice())
                .map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;

            TransactionKind::StorageWithdraw(args)
        }
        TransactionKindTag::SetPausedFlags => {
            let args = connector::PauseEthConnectorArgs::try_from_slice(&bytes).map_err(f)?;
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
                let args = connector::ExitToNearPrecompileCallbackArgs::try_from_slice(&bytes)
                    .map_err(f)?;
                TransactionKind::ExitToNear(Some(args))
            }
        }
        TransactionKindTag::SetConnectorData => {
            let args = connector::SetContractDataCallArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetConnectorData(args)
        }
        TransactionKindTag::NewConnector => {
            let args = connector::InitCallArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::NewConnector(args)
        }
        TransactionKindTag::NewEngine => {
            let args = engine::NewCallArgs::deserialize(&bytes).map_err(|e| {
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
            let args = engine::SetUpgradeDelayBlocksArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetUpgradeDelayBlocks(args)
        }
        TransactionKindTag::FundXccSubAccount => {
            let args = xcc::FundXccArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::FundXccSubAccount(args)
        }
        TransactionKindTag::PauseContract => TransactionKind::PauseContract,
        TransactionKindTag::ResumeContract => TransactionKind::ResumeContract,
        TransactionKindTag::SetKeyManager => {
            let args: engine::RelayerKeyManagerArgs = serde_json::from_slice(bytes.as_slice())
                .map_err(|e| {
                    ParseTransactionKindError::failed_deserialization(tx_kind_tag, Some(e))
                })?;
            TransactionKind::SetKeyManager(args)
        }
        TransactionKindTag::AddRelayerKey => {
            let args = engine::RelayerKeyArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::AddRelayerKey(args)
        }
        TransactionKindTag::StoreRelayerKeyCallback => {
            let args = engine::RelayerKeyArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::StoreRelayerKeyCallback(args)
        }
        TransactionKindTag::RemoveRelayerKey => {
            let args = engine::RelayerKeyArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::RemoveRelayerKey(args)
        }
        TransactionKindTag::StartHashchain => {
            let args = engine::StartHashchainArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::StartHashchain(args)
        }
        TransactionKindTag::SetErc20Metadata => {
            let args: connector::SetErc20MetadataArgs =
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
            let args =
                connector::SetEthConnectorContractAccountArgs::try_from_slice(&bytes).map_err(f)?;
            TransactionKind::SetEthConnectorContractAccount(args)
        }
        TransactionKindTag::MirrorErc20TokenCallback => {
            let args = connector::MirrorErc20TokenArgs::try_from_slice(&bytes).map_err(f)?;
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
            let mut context = storage
                .get_custom_data(b"more_context")?
                .and_then(|data| data.try_into().ok())
                .map_or_else(Context::initial, Context::deserialize);

            let mut transaction_message = *transaction_message;
            let raw_input = mem::take(&mut transaction_message.raw_input);
            let mut outcome = storage
                .with_engine_access(block_height, transaction_position, &raw_input, |io| {
                    execute_transaction(
                        transaction_message,
                        block_height,
                        &block_metadata,
                        engine_account_id,
                        None,
                        io,
                        &mut context,
                        EngineStateAccess::get_transaction_diff,
                    )
                })
                .result;
            outcome.info.raw_input = raw_input;
            storage.set_custom_data(b"more_context", &context.serialize())?;

            Ok(ConsumeMessageOutcome::TransactionIncluded(Box::new(
                outcome,
            )))
        }
    }
}

#[derive(Clone, Copy)]
pub enum TraceKind {
    Transaction,
    CallFrame,
}

pub fn execute_transaction_message<M: ModExpAlgorithm + 'static>(
    storage: &Storage,
    mut transaction_message: TransactionMessage,
    trace_kind: Option<TraceKind>,
) -> Result<TransactionIncludedOutcome, crate::Error> {
    let transaction_position = transaction_message.position;
    let block_hash = transaction_message.block_hash;
    let block_height = storage.get_block_height_by_hash(block_hash)?;
    let block_metadata = storage.get_block_metadata(block_hash)?;
    let engine_account_id = storage.get_engine_account_id()?;
    let mut context = storage
        .get_custom_data(b"more_context")?
        .and_then(|data| data.try_into().ok())
        .map_or_else(Context::initial, Context::deserialize);
    let raw_input = mem::take(&mut transaction_message.raw_input);
    let mut result =
        storage.with_engine_access(block_height, transaction_position, &raw_input, |io| {
            execute_transaction(
                transaction_message,
                block_height,
                &block_metadata,
                engine_account_id,
                trace_kind,
                io,
                &mut context,
                EngineStateAccess::get_transaction_diff,
            )
        });
    result.result.info.raw_input = raw_input;
    storage.set_custom_data(b"more_context", &context.serialize())?;
    Ok(result.result)
}

#[allow(clippy::too_many_arguments)]
pub fn execute_transaction<I, F>(
    transaction_message: TransactionMessage,
    block_height: u64,
    block_metadata: &BlockMetadata,
    engine_account_id: AccountId,
    trace_kind: Option<TraceKind>,
    mut io: I,
    context: &mut Context,
    get_diff: F,
) -> TransactionIncludedOutcome
where
    I: IO + Send + Copy,
    I::StorageValue: AsRef<[u8]>,
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

    // TODO: load code dynamically and check hash
    let code = include_bytes!("../../../bin/aurora-engine-traced.wasm").to_vec();
    let runner = ContractRunner::new(code, None);

    let promise_results = transaction_message
        .promise_data
        .iter()
        .cloned()
        .map(|data| data.map_or(PromiseResult::Failed, PromiseResult::Successful))
        .collect::<Vec<_>>()
        .into();

    let (tx_hash, result, trace_log, call_tracer) = match &transaction_message.transaction {
        TransactionKind::Submit(tx) => {
            // We can ignore promises in the standalone engine because it processes each receipt separately
            // and it is fed a stream of receipts (it does not schedule them)
            let tx_data: Vec<u8> = tx.into();
            let tx_hash = aurora_engine_sdk::keccak(&tx_data);
            let method = match trace_kind {
                None => "submit",
                Some(TraceKind::Transaction) => "submit_trace_tx",
                Some(TraceKind::CallFrame) => "submit_trace_call",
            };
            let mut trace_log = None;
            let mut trace_call_stack = None;
            let result = runner
                .call_helper(method, promise_results, &env, io, context, None)
                .map_err(ExecutionError::from)
                .and_then(|data| {
                    data.map(|data| {
                        let mut slice = data.as_slice();
                        io.return_output(&data);
                        let res = SubmitResult::deserialize_reader(&mut slice)
                            .map_err(ExecutionError::Deserialize)?;
                        if !slice.is_empty() {
                            match trace_kind {
                                Some(TraceKind::Transaction) => {
                                    trace_log =
                                        Logs::deserialize_reader(&mut slice).ok().map(|Logs(l)| l);
                                }
                                Some(TraceKind::CallFrame) => {
                                    trace_call_stack =
                                        CallTracer::deserialize_reader(&mut slice).ok();
                                }
                                None => {}
                            }
                        }
                        Ok(TransactionExecutionResult::Submit(Ok(res)))
                    })
                    .transpose()
                });
            (tx_hash, result, trace_log, trace_call_stack)
        }
        TransactionKind::SubmitWithArgs(args) => {
            let tx_hash = aurora_engine_sdk::keccak(&args.tx_data);
            let result = runner
                .call_helper("submit", promise_results, &env, io, context, None)
                .map_err(ExecutionError::from)
                .and_then(|data| {
                    data.map(|data| {
                        io.return_output(&data);
                        let res = SubmitResult::try_from_slice(&data)
                            .map_err(ExecutionError::Deserialize)?;
                        Ok(TransactionExecutionResult::Submit(Ok(res)))
                    })
                    .transpose()
                });

            (tx_hash, result, None, None)
        }
        other => {
            let result = non_submit_execute(other, &runner, io, &env, promise_results, context);
            (near_receipt_id, result, None, None)
        }
    };

    let diff = get_diff(&io);

    TransactionIncludedOutcome {
        hash: tx_hash,
        info: transaction_message,
        diff,
        maybe_result: result,
        trace_log,
        call_tracer,
    }
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
fn non_submit_execute<I: IO + Send + Copy>(
    transaction: &TransactionKind,
    runner: &ContractRunner,
    io: I,
    env: &env::Fixed,
    promise_results: Arc<[PromiseResult]>,
    ctx: &mut Context,
) -> Result<Option<TransactionExecutionResult>, ExecutionError>
where
    I::StorageValue: AsRef<[u8]>,
{
    let result = match transaction {
        TransactionKind::Call(_) => {
            // We can ignore promises in the standalone engine (see above)
            let data = runner
                .call_helper("call", promise_results, env, io, ctx, None)?
                .ok_or(ExecutionError::DeserializeUnexpectedEnd)?;
            let result =
                SubmitResult::try_from_slice(&data).map_err(ExecutionError::Deserialize)?;

            Some(TransactionExecutionResult::Submit(Ok(result)))
        }

        TransactionKind::Deploy(_) => {
            let data = runner
                .call_helper("deploy_code", promise_results, env, io, ctx, None)?
                .ok_or(ExecutionError::DeserializeUnexpectedEnd)?;
            let result =
                SubmitResult::try_from_slice(&data).map_err(ExecutionError::Deserialize)?;

            Some(TransactionExecutionResult::Submit(Ok(result)))
        }
        TransactionKind::DeployErc20(_) => {
            let data =
                runner.call_helper("deploy_erc20_token", promise_results, env, io, ctx, None)?;

            Some(match data {
                Some(data) => {
                    let mut slice = data.as_slice();
                    let address =
                        Address::deserialize(&mut slice).map_err(ExecutionError::Deserialize)?;
                    TransactionExecutionResult::DeployErc20(address)
                }
                None => {
                    //
                    // TransactionExecutionResult::Promise(promise_args)
                    panic!("cannot handle case where `deploy_erc20_token` returns promise")
                }
            })
        }
        TransactionKind::DeployErc20Callback(_) => {
            let data = runner
                .call_helper(
                    "deploy_erc20_token_callback",
                    promise_results,
                    env,
                    io,
                    ctx,
                    None,
                )?
                .ok_or(ExecutionError::DeserializeUnexpectedEnd)?;
            let mut slice = data.as_slice();
            let address = Address::deserialize(&mut slice).map_err(ExecutionError::Deserialize)?;

            Some(TransactionExecutionResult::DeployErc20(address))
        }
        TransactionKind::FtOnTransfer(_) => {
            let data = runner
                .call_helper(
                    "ft_on_transfer_with_return",
                    promise_results,
                    env,
                    io,
                    ctx,
                    None,
                )?
                .ok_or(ExecutionError::DeserializeUnexpectedEnd)?;
            let submit_result = Option::<SubmitResult>::try_from_slice(&data)
                .map_err(ExecutionError::Deserialize)?;

            submit_result.map(|result| TransactionExecutionResult::Submit(Ok(result)))
        }
        TransactionKind::FtTransferCall(_) => None,
        TransactionKind::ResolveTransfer(_, _) => None,
        TransactionKind::FtTransfer(_) => None,
        TransactionKind::Withdraw(_) => None,
        TransactionKind::Deposit(_) => None,
        TransactionKind::FinishDeposit(_) => None,
        TransactionKind::StorageDeposit(_) => None,
        TransactionKind::StorageUnregister(_) => None,
        TransactionKind::StorageWithdraw(_) => None,
        TransactionKind::SetPausedFlags(_) => None,
        TransactionKind::RegisterRelayer(_) => {
            runner.call_helper("register_relayer", promise_results, env, io, ctx, None)?;

            None
        }
        TransactionKind::ExitToNear(_) => {
            runner.call_helper(
                "exit_to_near_precompile_callback",
                promise_results,
                env,
                io,
                ctx,
                None,
            )?;

            // maybe_result.map(|submit_result| TransactionExecutionResult::Submit(Ok(submit_result)))
            None
        }
        TransactionKind::SetConnectorData(_) => None,
        TransactionKind::NewConnector(_) => None,
        TransactionKind::NewEngine(_) => {
            runner.call_helper("new", promise_results, env, io, ctx, None)?;
            None
        }
        TransactionKind::SetEthConnectorContractAccount(_) => {
            runner.call_helper(
                "set_eth_connector_contract_account",
                promise_results,
                env,
                io,
                ctx,
                None,
            )?;

            None
        }
        TransactionKind::FactoryUpdate(_) => {
            runner.call_helper("factory_update", promise_results, env, io, ctx, None)?;

            None
        }
        TransactionKind::FactoryUpdateAddressVersion(_) => {
            runner.call_helper(
                "factory_update_address_version",
                promise_results,
                env,
                io,
                ctx,
                None,
            )?;

            None
        }
        TransactionKind::FactorySetWNearAddress(_) => {
            runner.call_helper(
                "factory_set_wnear_address",
                promise_results,
                env,
                io,
                ctx,
                None,
            )?;

            None
        }
        TransactionKind::FundXccSubAccount(_) => {
            runner.call_helper("fund_xcc_sub_account", promise_results, env, io, ctx, None)?;

            None
        }
        TransactionKind::WithdrawWnearToRouter(_) => {
            let data = runner
                .call_helper(
                    "withdraw_wnear_to_router",
                    promise_results,
                    env,
                    io,
                    ctx,
                    None,
                )?
                .ok_or(ExecutionError::DeserializeUnexpectedEnd)?;
            let result =
                SubmitResult::try_from_slice(&data).map_err(ExecutionError::Deserialize)?;

            Some(TransactionExecutionResult::Submit(Ok(result)))
        }
        TransactionKind::Unknown => None,
        // Not handled in this function; is handled by the general `execute_transaction` function
        TransactionKind::Submit(_) | TransactionKind::SubmitWithArgs(_) => unreachable!(),
        TransactionKind::PausePrecompiles(_) => {
            runner.call_helper("pause_precompiles", promise_results, env, io, ctx, None)?;

            None
        }
        TransactionKind::ResumePrecompiles(_) => {
            runner.call_helper("resume_precompiles", promise_results, env, io, ctx, None)?;

            None
        }
        TransactionKind::SetOwner(_) => {
            runner.call_helper("set_owner", promise_results, env, io, ctx, None)?;

            None
        }
        TransactionKind::SetUpgradeDelayBlocks(_) => {
            runner.call_helper(
                "set_upgrade_delay_blocks",
                promise_results,
                env,
                io,
                ctx,
                None,
            )?;

            None
        }
        TransactionKind::PauseContract => {
            runner.call_helper("pause_contract", promise_results, env, io, ctx, None)?;

            None
        }
        TransactionKind::ResumeContract => {
            runner.call_helper("resume_contract", promise_results, env, io, ctx, None)?;

            None
        }
        TransactionKind::SetKeyManager(_) => {
            runner.call_helper("set_key_manager", promise_results, env, io, ctx, None)?;

            None
        }
        TransactionKind::AddRelayerKey(_) => {
            runner.call_helper("add_relayer_key", promise_results, env, io, ctx, None)?;

            None
        }
        TransactionKind::StoreRelayerKeyCallback(_) => {
            runner.call_helper(
                "store_relayer_key_callback",
                promise_results,
                env,
                io,
                ctx,
                None,
            )?;

            None
        }
        TransactionKind::RemoveRelayerKey(_) => {
            runner.call_helper("remove_relayer_key", promise_results, env, io, ctx, None)?;

            None
        }
        TransactionKind::StartHashchain(_) => {
            runner.call_helper("start_hashchain", promise_results, env, io, ctx, None)?;

            None
        }
        TransactionKind::SetErc20Metadata(_) => {
            runner.call_helper("set_erc20_metadata", promise_results, env, io, ctx, None)?;

            None
        }
        TransactionKind::SetFixedGas(args) => {
            let input = borsh::to_vec(args).map_err(ExecutionError::SerializeArg)?;
            runner.call_helper("set_fixed_gas", promise_results, env, io, ctx, Some(input))?;

            None
        }
        TransactionKind::SetErc20FallbackAddress(args) => {
            let input = borsh::to_vec(args).map_err(ExecutionError::SerializeArg)?;
            runner.call_helper(
                "set_erc20_fallback_address",
                promise_results,
                env,
                io,
                ctx,
                Some(input),
            )?;

            None
        }
        TransactionKind::SetSiloParams(args) => {
            let input = borsh::to_vec(args).map_err(ExecutionError::SerializeArg)?;
            runner.call_helper(
                "set_silo_params",
                promise_results,
                env,
                io,
                ctx,
                Some(input),
            )?;

            None
        }
        TransactionKind::AddEntryToWhitelist(args) => {
            let input = borsh::to_vec(args).map_err(ExecutionError::SerializeArg)?;
            runner.call_helper(
                "add_entry_to_whitelist",
                promise_results,
                env,
                io,
                ctx,
                Some(input),
            )?;

            None
        }
        TransactionKind::AddEntryToWhitelistBatch(args) => {
            let input = borsh::to_vec(args).map_err(ExecutionError::SerializeArg)?;
            runner.call_helper(
                "add_entry_to_whitelist_batch",
                promise_results,
                env,
                io,
                ctx,
                Some(input),
            )?;

            None
        }
        TransactionKind::RemoveEntryFromWhitelist(args) => {
            let input = borsh::to_vec(args).map_err(ExecutionError::SerializeArg)?;
            runner.call_helper(
                "remove_entry_from_whitelist",
                promise_results,
                env,
                io,
                ctx,
                Some(input),
            )?;

            None
        }
        TransactionKind::SetWhitelistStatus(args) => {
            let input = borsh::to_vec(args).map_err(ExecutionError::SerializeArg)?;
            runner.call_helper(
                "set_whitelist_status",
                promise_results,
                env,
                io,
                ctx,
                Some(input),
            )?;

            None
        }
        TransactionKind::SetWhitelistsStatuses(args) => {
            let input = borsh::to_vec(args).map_err(ExecutionError::SerializeArg)?;
            runner.call_helper(
                "set_whitelists_statuses",
                promise_results,
                env,
                io,
                ctx,
                Some(input),
            )?;

            None
        }
        TransactionKind::MirrorErc20TokenCallback(_) => {
            runner.call_helper(
                "mirror_erc20_token_callback",
                promise_results,
                env,
                io,
                ctx,
                None,
            )?;

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
    pub maybe_result: Result<Option<TransactionExecutionResult>, ExecutionError>,
    pub trace_log: Option<Vec<TraceLog>>,
    pub call_tracer: Option<CallTracer>,
}

impl TransactionIncludedOutcome {
    pub fn commit(&self, storage: &mut Storage) -> Result<(), crate::error::Error> {
        match self.maybe_result.as_ref() {
            Err(_) | Ok(Some(TransactionExecutionResult::Submit(Err(_)))) => (), // do not persist if Engine encounters an error
            _ => storage.set_transaction_included(self.hash, &self.info, &self.diff)?,
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionExecutionResult {
    Submit(aurora_engine::engine::EngineResult<SubmitResult>),
    DeployErc20(Address),
    Promise(PromiseWithCallbackArgs),
}

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("{0:?}")]
    VMRunnerError(Box<dyn Debug + Send + Sync + 'static>),
    #[error("engine: {0:?}")]
    Engine(EngineError),
    #[error("serialize arguments: {0}")]
    SerializeArg(io::Error),
    #[error("deserialize: {0}")]
    Deserialize(io::Error),
    #[error("deserialize: unexpected end of stream")]
    DeserializeUnexpectedEnd,
}

impl From<VMRunnerError> for ExecutionError {
    fn from(value: VMRunnerError) -> Self {
        Self::VMRunnerError(Box::new(value))
    }
}

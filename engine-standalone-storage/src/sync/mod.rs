use std::fmt::Debug;
use std::io;

use aurora_engine::engine::EngineError;
use aurora_engine::parameters::SubmitResult;
use aurora_engine_modexp::ModExpAlgorithm;
use aurora_engine_sdk::{env, io::IO};
use aurora_engine_types::borsh;
use aurora_engine_types::types::NearGas;
use aurora_engine_types::{
    account_id::AccountId,
    borsh::BorshDeserialize,
    parameters::{silo as silo_params, PromiseWithCallbackArgs},
    types::Address,
    H256,
};
use engine_standalone_tracing::types::call_tracer::CallTracer;
use engine_standalone_tracing::{Logs, TraceLog};
use thiserror::Error;

pub mod types;

use crate::engine_state::EngineStateAccess;
use crate::runner::AbstractContractRunner;
use crate::{BlockMetadata, Diff, Storage};
use types::{Message, TransactionKind, TransactionKindTag, TransactionMessage};

/// Note: this function does not automatically commit transaction messages to the storage.
/// If you want the transaction diff committed then you must call the `commit` method on
/// the outcome of this function.
pub fn consume_message<M: ModExpAlgorithm + 'static, R>(
    storage: &mut Storage,
    runner: &R,
    message: Message,
) -> Result<ConsumeMessageOutcome, crate::Error>
where
    R: AbstractContractRunner,
    R::Error: Debug + Send + Sync + 'static,
{
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

            let transaction_message = *transaction_message;
            let raw_input = transaction_message.transaction.args.clone();
            let outcome = storage
                .with_engine_access(block_height, transaction_position, &raw_input, |io| {
                    execute_transaction(
                        runner,
                        transaction_message,
                        block_height,
                        &block_metadata,
                        engine_account_id,
                        None,
                        io,
                        EngineStateAccess::get_transaction_diff,
                    )
                })
                .result;

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

pub fn execute_transaction_message<M: ModExpAlgorithm + 'static, R>(
    storage: &Storage,
    runner: &R,
    transaction_message: TransactionMessage,
    trace_kind: Option<TraceKind>,
) -> Result<TransactionIncludedOutcome, crate::Error>
where
    R: AbstractContractRunner,
    R::Error: Debug + Send + Sync + 'static,
{
    let transaction_position = transaction_message.position;
    let block_hash = transaction_message.block_hash;
    let block_height = storage.get_block_height_by_hash(block_hash)?;
    let block_metadata = storage.get_block_metadata(block_hash)?;
    let engine_account_id = storage.get_engine_account_id()?;
    let raw_input = transaction_message.transaction.args.clone();
    let result = storage.with_engine_access(block_height, transaction_position, &raw_input, |io| {
        execute_transaction(
            runner,
            transaction_message,
            block_height,
            &block_metadata,
            engine_account_id,
            trace_kind,
            io,
            EngineStateAccess::get_transaction_diff,
        )
    });
    Ok(result.result)
}

#[allow(clippy::too_many_arguments)]
pub fn execute_transaction<I, F, R>(
    runner: &R,
    transaction_message: TransactionMessage,
    block_height: u64,
    block_metadata: &BlockMetadata,
    engine_account_id: AccountId,
    trace_kind: Option<TraceKind>,
    mut io: I,
    get_diff: F,
) -> TransactionIncludedOutcome
where
    I: IO + Send + Copy,
    I::StorageValue: AsRef<[u8]>,
    F: FnOnce(&I) -> Diff,
    R: AbstractContractRunner,
    R::Error: Debug + Send + Sync + 'static,
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
        prepaid_gas: transaction_message.prepaid_gas,
        used_gas: NearGas::new(0),
    };

    let promise_data = transaction_message.promise_data.clone();

    let (tx_hash, result, trace_log, call_tracer) =
        match &transaction_message.transaction.method_name {
            TransactionKindTag::Submit => {
                // We can ignore promises in the standalone engine because it processes each receipt separately
                // and it is fed a stream of receipts (it does not schedule them)

                let tx_hash = aurora_engine_sdk::keccak(&transaction_message.transaction.args);
                let method = match trace_kind {
                    None => "submit",
                    Some(TraceKind::Transaction) => "submit_trace_tx",
                    Some(TraceKind::CallFrame) => "submit_trace_call",
                };
                let mut trace_log = None;
                let mut trace_call_stack = None;
                let result = runner
                    .call_contract(method, promise_data, &env, io, None)
                    .map_err(ExecutionError::from_vm_err)
                    .and_then(|data| {
                        data.map(|data| {
                            let mut slice = data.as_slice();
                            io.return_output(&data);
                            let res = SubmitResult::deserialize_reader(&mut slice)
                                .map_err(ExecutionError::Deserialize)?;
                            if !slice.is_empty() {
                                match trace_kind {
                                    Some(TraceKind::Transaction) => {
                                        trace_log = Logs::deserialize_reader(&mut slice)
                                            .ok()
                                            .map(|Logs(l)| l);
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
            TransactionKindTag::SubmitWithArgs => {
                let args = transaction_message.transaction.get_submit_args().unwrap();

                let tx_hash = aurora_engine_sdk::keccak(&args.tx_data);
                let result = runner
                    .call_contract("submit_with_args", promise_data, &env, io, None)
                    .map_err(ExecutionError::from_vm_err)
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
            _ => {
                let result = non_submit_execute(
                    &transaction_message.transaction,
                    runner,
                    io,
                    &env,
                    promise_data,
                );
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
fn non_submit_execute<I: IO + Send + Copy, R>(
    transaction: &TransactionKind,
    runner: &R,
    io: I,
    env: &env::Fixed,
    promise_results: Vec<Option<Vec<u8>>>,
) -> Result<Option<TransactionExecutionResult>, ExecutionError>
where
    I::StorageValue: AsRef<[u8]>,
    R: AbstractContractRunner,
    R::Error: Debug + Send + Sync + 'static,
{
    let result = match transaction.method_name {
        TransactionKindTag::Call => {
            // We can ignore promises in the standalone engine (see above)
            let data = runner
                .call_contract("call", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?
                .ok_or(ExecutionError::DeserializeUnexpectedEnd)?;
            let result =
                SubmitResult::try_from_slice(&data).map_err(ExecutionError::Deserialize)?;

            Some(TransactionExecutionResult::Submit(Ok(result)))
        }

        TransactionKindTag::Deploy => {
            let data = runner
                .call_contract("deploy_code", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?
                .ok_or(ExecutionError::DeserializeUnexpectedEnd)?;
            let result =
                SubmitResult::try_from_slice(&data).map_err(ExecutionError::Deserialize)?;

            Some(TransactionExecutionResult::Submit(Ok(result)))
        }
        TransactionKindTag::DeployErc20 => {
            let data = runner
                .call_contract("deploy_erc20_token", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            match data {
                Some(data) => {
                    let mut slice = data.as_slice();
                    let address =
                        Address::deserialize(&mut slice).map_err(ExecutionError::Deserialize)?;
                    Some(TransactionExecutionResult::DeployErc20(address))
                }
                None => None,
            }
        }
        TransactionKindTag::DeployErc20Callback => {
            let data = runner
                .call_contract(
                    "deploy_erc20_token_callback",
                    promise_results,
                    env,
                    io,
                    None,
                )
                .map_err(ExecutionError::from_vm_err)?
                .ok_or(ExecutionError::DeserializeUnexpectedEnd)?;
            let mut slice = data.as_slice();
            let address = Address::deserialize(&mut slice).map_err(ExecutionError::Deserialize)?;

            Some(TransactionExecutionResult::DeployErc20(address))
        }
        TransactionKindTag::FtOnTransfer => {
            let data = runner
                .call_contract(
                    "borealis_wrapper_ft_on_transfer",
                    promise_results,
                    env,
                    io,
                    None,
                )
                .map_err(ExecutionError::from_vm_err)?
                .ok_or(ExecutionError::DeserializeUnexpectedEnd)?;
            let submit_result = Option::<SubmitResult>::try_from_slice(&data)
                .map_err(ExecutionError::Deserialize)?;

            submit_result.map(|result| TransactionExecutionResult::Submit(Ok(result)))
        }
        TransactionKindTag::FtTransferCall => None,
        TransactionKindTag::ResolveTransfer => None,
        TransactionKindTag::FtTransfer => None,
        TransactionKindTag::Withdraw => None,
        TransactionKindTag::Deposit => None,
        TransactionKindTag::FinishDeposit => None,
        TransactionKindTag::StorageDeposit => None,
        TransactionKindTag::StorageUnregister => None,
        TransactionKindTag::StorageWithdraw => None,
        TransactionKindTag::SetPausedFlags => None,
        TransactionKindTag::RegisterRelayer => {
            runner
                .call_contract("register_relayer", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::ExitToNear => runner
            .call_contract(
                "borealis_wrapper_exit_to_near_precompile_callback",
                promise_results,
                env,
                io,
                None,
            )
            .map_err(ExecutionError::from_vm_err)?
            .map(|data| Option::try_from_slice(&data).map_err(ExecutionError::Deserialize))
            .transpose()?
            .flatten()
            .map(Ok)
            .map(TransactionExecutionResult::Submit),
        TransactionKindTag::SetConnectorData => None,
        TransactionKindTag::NewConnector => None,
        TransactionKindTag::NewEngine => {
            runner
                .call_contract("new", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;
            None
        }
        TransactionKindTag::SetEthConnectorContractAccount => {
            runner
                .call_contract(
                    "set_eth_connector_contract_account",
                    promise_results,
                    env,
                    io,
                    None,
                )
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::FactoryUpdate => {
            runner
                .call_contract("factory_update", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::FactoryUpdateAddressVersion => {
            runner
                .call_contract(
                    "factory_update_address_version",
                    promise_results,
                    env,
                    io,
                    None,
                )
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::FactorySetWNearAddress => {
            runner
                .call_contract("factory_set_wnear_address", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::FundXccSubAccount => {
            runner
                .call_contract("fund_xcc_sub_account", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::WithdrawWnearToRouter => {
            let data = runner
                .call_contract("withdraw_wnear_to_router", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?
                .ok_or(ExecutionError::DeserializeUnexpectedEnd)?;
            let result =
                SubmitResult::try_from_slice(&data).map_err(ExecutionError::Deserialize)?;

            Some(TransactionExecutionResult::Submit(Ok(result)))
        }
        TransactionKindTag::Unknown => None,
        // Not handled in this function; is handled by the general `execute_transaction` function
        TransactionKindTag::Submit | TransactionKindTag::SubmitWithArgs => unreachable!(),
        TransactionKindTag::PausePrecompiles => {
            runner
                .call_contract("pause_precompiles", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::ResumePrecompiles => {
            runner
                .call_contract("resume_precompiles", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::SetOwner => {
            runner
                .call_contract("set_owner", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::SetUpgradeDelayBlocks => {
            runner
                .call_contract("set_upgrade_delay_blocks", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::PauseContract => {
            runner
                .call_contract("pause_contract", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::ResumeContract => {
            runner
                .call_contract("resume_contract", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::SetKeyManager => {
            runner
                .call_contract("set_key_manager", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::AddRelayerKey => {
            runner
                .call_contract("add_relayer_key", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::StoreRelayerKeyCallback => {
            runner
                .call_contract("store_relayer_key_callback", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::RemoveRelayerKey => {
            runner
                .call_contract("remove_relayer_key", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::StartHashchain => {
            runner
                .call_contract("start_hashchain", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::SetErc20Metadata => {
            runner
                .call_contract("set_erc20_metadata", promise_results, env, io, None)
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::SetFixedGas => {
            let args = silo_params::FixedGasArgs::try_from_slice(&transaction.args).unwrap();
            let input = borsh::to_vec(&args).map_err(ExecutionError::SerializeArg)?;
            runner
                .call_contract("set_fixed_gas", promise_results, env, io, Some(input))
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::SetErc20FallbackAddress => {
            let args =
                silo_params::Erc20FallbackAddressArgs::try_from_slice(&transaction.args).unwrap();
            let input = borsh::to_vec(&args).map_err(ExecutionError::SerializeArg)?;
            runner
                .call_contract(
                    "set_erc20_fallback_address",
                    promise_results,
                    env,
                    io,
                    Some(input),
                )
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::SetSiloParams => {
            let args: Option<silo_params::SiloParamsArgs> =
                BorshDeserialize::try_from_slice(&transaction.args).unwrap();
            let input = borsh::to_vec(&args).map_err(ExecutionError::SerializeArg)?;
            runner
                .call_contract("set_silo_params", promise_results, env, io, Some(input))
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::AddEntryToWhitelist => {
            let args = silo_params::WhitelistArgs::try_from_slice(&transaction.args).unwrap();
            let input = borsh::to_vec(&args).map_err(ExecutionError::SerializeArg)?;
            runner
                .call_contract(
                    "add_entry_to_whitelist",
                    promise_results,
                    env,
                    io,
                    Some(input),
                )
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::AddEntryToWhitelistBatch => {
            let args: Vec<silo_params::WhitelistArgs> =
                BorshDeserialize::try_from_slice(&transaction.args).unwrap();
            let input = borsh::to_vec(&args).map_err(ExecutionError::SerializeArg)?;
            runner
                .call_contract(
                    "add_entry_to_whitelist_batch",
                    promise_results,
                    env,
                    io,
                    Some(input),
                )
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::RemoveEntryFromWhitelist => {
            let args = silo_params::WhitelistArgs::try_from_slice(&transaction.args).unwrap();
            let input = borsh::to_vec(&args).map_err(ExecutionError::SerializeArg)?;
            runner
                .call_contract(
                    "remove_entry_from_whitelist",
                    promise_results,
                    env,
                    io,
                    Some(input),
                )
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::SetWhitelistStatus => {
            let args = silo_params::WhitelistStatusArgs::try_from_slice(&transaction.args).unwrap();
            let input = borsh::to_vec(&args).map_err(ExecutionError::SerializeArg)?;
            runner
                .call_contract(
                    "set_whitelist_status",
                    promise_results,
                    env,
                    io,
                    Some(input),
                )
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::SetWhitelistsStatuses => {
            let args: Vec<silo_params::WhitelistStatusArgs> =
                BorshDeserialize::try_from_slice(&transaction.args).unwrap();
            let input = borsh::to_vec(&args).map_err(ExecutionError::SerializeArg)?;
            runner
                .call_contract(
                    "set_whitelists_statuses",
                    promise_results,
                    env,
                    io,
                    Some(input),
                )
                .map_err(ExecutionError::from_vm_err)?;

            None
        }
        TransactionKindTag::MirrorErc20TokenCallback => {
            runner
                .call_contract(
                    "mirror_erc20_token_callback",
                    promise_results,
                    env,
                    io,
                    None,
                )
                .map_err(ExecutionError::from_vm_err)?;

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

impl ExecutionError {
    fn from_vm_err<E>(value: E) -> Self
    where
        E: Debug + Send + Sync + 'static,
    {
        Self::VMRunnerError(Box::new(value))
    }
}

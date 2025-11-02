use std::fmt::Debug;
use std::io;

use aurora_engine_modexp::ModExpAlgorithm;
use aurora_engine_sdk::io::StorageIntermediate;
use aurora_engine_sdk::{env, io::IO};
use aurora_engine_types::types::NearGas;
use aurora_engine_types::{
    account_id::AccountId, borsh::BorshDeserialize, parameters::engine::TransactionExecutionResult,
    H256,
};
use engine_standalone_tracing::types::call_tracer::CallTracer;
use engine_standalone_tracing::{TraceKind, TraceLog};
use thiserror::Error;

pub mod types;

use crate::engine_state::EngineStateAccess;
use crate::runner::AbstractContractRunner;
use crate::{BlockMetadata, Diff, Storage};
use types::{Message, TransactionMessage};

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

    // We can ignore promises in the standalone engine because it processes each receipt separately
    // and it is fed a stream of receipts (it does not schedule them)
    let promise_data = transaction_message.promise_data.clone();

    if let Some(v) = &trace_kind {
        io.write_borsh(b"borealis/trace_kind", v);
    }

    let tx_hash = match transaction_message.transaction.method_name.as_str() {
        "submit" => aurora_engine_sdk::keccak(&transaction_message.transaction.args),
        "submit_with_args" => {
            let args = transaction_message.transaction.get_submit_args().unwrap();
            aurora_engine_sdk::keccak(&args.tx_data)
        }
        _ => near_receipt_id,
    };

    io.write_borsh(
        b"borealis/method",
        &transaction_message.transaction.method_name,
    );
    let maybe_result = runner
        .call_contract(promise_data, &env, io)
        .map_err(ExecutionError::from_vm_err)
        .and_then(|_| {
            type R = Result<Option<TransactionExecutionResult>, String>;

            let value = io.read_storage(b"borealis/result");
            let value = value.ok_or_else(|| ExecutionError::DeserializeUnexpectedEnd)?;
            let value = value.to_vec();
            let mut value_slice = value.as_slice();
            R::deserialize(&mut value_slice)
                .map_err(ExecutionError::Deserialize)?
                .map_err(ExecutionError::Inner)
        });

    let value = io.read_storage(b"borealis/transaction_tracing");
    let trace_log = value.and_then(|v| v.to_value().ok());
    let value = io.read_storage(b"borealis/call_frame_tracing");
    let call_tracer = value.and_then(|v| v.to_value().ok());
    let value = io.read_storage(b"borealis/custom_debug_info");
    let custom_debug_info = value.map(|x| x.as_ref().to_vec());

    let diff = get_diff(&io);

    TransactionIncludedOutcome {
        hash: tx_hash,
        info: transaction_message,
        diff,
        maybe_result,
        trace_log,
        call_tracer,
        custom_debug_info,
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
    pub custom_debug_info: Option<Vec<u8>>,
}

impl TransactionIncludedOutcome {
    pub fn commit(&self, storage: &mut Storage) -> Result<(), crate::error::Error> {
        // do not persist if Engine encounters an error
        if self.maybe_result.is_ok() {
            storage.set_transaction_included(self.hash, &self.info, &self.diff)?;
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("{0:?}")]
    VMRunnerError(Box<dyn Debug + Send + Sync + 'static>),
    #[error("{0}")]
    Inner(String),
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

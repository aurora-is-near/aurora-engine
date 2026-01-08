use std::fmt::Debug;

use aurora_engine_sdk::env;
use aurora_engine_types::types::NearGas;
use aurora_engine_types::{parameters::engine::TransactionExecutionResult, H256};
use engine_standalone_tracing::types::call_tracer::CallTracer;
use engine_standalone_tracing::TraceLog;

pub mod types;

use crate::wasmer_runner::WasmerRuntimeOutcome;
use crate::{Diff, Storage};
use types::{BlockMessage, Message, TransactionMessage};

pub fn consume_message<const KEEP_DIFF: bool>(
    storage: &mut Storage,
    message: Message,
) -> Result<ConsumeMessageOutcome, crate::Error> {
    match message {
        Message::Block(msg) => {
            consume_block_message(storage, msg).map(|()| ConsumeMessageOutcome::BlockAdded)
        }
        Message::Transaction(msg) => execute_transaction_message::<KEEP_DIFF>(storage, *msg)
            .map(Box::new)
            .map(ConsumeMessageOutcome::TransactionIncluded),
    }
}

fn consume_block_message(
    storage: &mut Storage,
    block_message: BlockMessage,
) -> Result<(), crate::Error> {
    let block_hash = block_message.hash;
    let block_height = block_message.height;
    let block_metadata = block_message.metadata;
    storage
        .set_block_data(block_hash, block_height, &block_metadata)
        .map_err(crate::Error::Rocksdb)
}

/// Note: this function does not automatically commit transaction messages to the storage.
/// If you want the transaction diff committed then you must call the `commit` method on
/// the outcome of this function.
///
/// If `KEEP_DIFF` is true the generated diff will remain cached even without commiting the outcome.
/// This is useful to batch transactions.
/// Need to take the cached diff at the end of the batch: `storage.runner_mut().take_cached_diff()`.
pub fn execute_transaction_message<const KEEP_DIFF: bool>(
    storage: &mut Storage,
    transaction_message: TransactionMessage,
) -> Result<TransactionIncludedOutcome, crate::Error> {
    let transaction_position = transaction_message.position;
    let block_hash = transaction_message.block_hash;
    let block_height = storage.get_block_height_by_hash(block_hash)?;
    let block_metadata = storage.get_block_metadata(block_hash)?;
    let engine_account_id = storage.get_engine_account_id()?;
    let raw_input = transaction_message.transaction.clone_raw_input();
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
        prepaid_gas: NearGas::new(u64::MAX),
        used_gas: NearGas::new(0),
    };

    let tx_hash = match transaction_message.transaction.method_name.as_str() {
        "submit" => aurora_engine_sdk::keccak(&transaction_message.transaction.args),
        "submit_with_args" => {
            let args = transaction_message.transaction.get_submit_args().unwrap();
            aurora_engine_sdk::keccak(&args.tx_data)
        }
        _ => near_receipt_id,
    };

    let WasmerRuntimeOutcome {
        diff,
        maybe_result,
        trace_log,
        call_tracer,
        custom_debug_info,
        ..
    } = storage
        .runner_mut()
        .call_contract(
            &transaction_message.transaction.method_name,
            transaction_message.trace_kind,
            &transaction_message.promise_data,
            env,
            block_height,
            transaction_position,
            raw_input,
        )
        .map_err(crate::Error::Wasmer)?;
    if !KEEP_DIFF {
        drop(storage.runner_mut().take_cached_diff());
    }

    Ok(TransactionIncludedOutcome {
        hash: tx_hash,
        info: transaction_message,
        diff,
        maybe_result,
        trace_log,
        call_tracer,
        custom_debug_info,
    })
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
    pub maybe_result: Result<Option<TransactionExecutionResult>, String>,
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

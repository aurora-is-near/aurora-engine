use aurora_engine::admin_controlled::AdminControlled;
use aurora_engine::pausables::{
    EnginePrecompilesPauser, PausedPrecompilesManager, PrecompileFlags,
};
use aurora_engine::{connector, engine, parameters::SubmitResult, xcc};
use aurora_engine_sdk::env::{self, Env, DEFAULT_PREPAID_GAS};
use aurora_engine_types::parameters::{EngineWithdrawCallArgs, PromiseCreateArgs};
use aurora_engine_types::{account_id::AccountId, types::Address, H256};

pub mod types;

use crate::engine_state::EngineStateAccess;
use crate::{BlockMetadata, Diff, Storage};
use types::{Message, TransactionKind, TransactionMessage};

pub fn consume_message(
    storage: &mut Storage,
    message: Message,
) -> Result<ConsumeMessageOutcome, crate::Error> {
    match message {
        Message::Block(block_message) => {
            let block_hash = block_message.hash;
            let block_height = block_message.height;
            let block_metadata = block_message.metadata;
            storage
                .set_block_data(block_hash, block_height, block_metadata)
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
                    execute_transaction(
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

pub fn execute_transaction_message(
    storage: &Storage,
    transaction_message: TransactionMessage,
) -> Result<TransactionIncludedOutcome, crate::Error> {
    let transaction_position = transaction_message.position;
    let block_hash = transaction_message.block_hash;
    let block_height = storage.get_block_height_by_hash(block_hash)?;
    let block_metadata = storage.get_block_metadata(block_hash)?;
    let engine_account_id = storage.get_engine_account_id()?;
    let result = storage.with_engine_access(block_height, transaction_position, &[], |io| {
        execute_transaction(
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

fn execute_transaction<'db>(
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
            let transaction_bytes: Vec<u8> = tx.into();
            let tx_hash = aurora_engine_sdk::keccak(&transaction_bytes);

            let result = engine::get_state(&io)
                .map(|engine_state| {
                    let submit_result = engine::submit(
                        io,
                        &env,
                        &transaction_bytes,
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
            let result = non_submit_execute(
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
/// The `submit` transaction kind is special because it is the only one where the transaction hash is
/// different than the NEAR receipt hash.
fn non_submit_execute<'db>(
    transaction: &TransactionKind,
    mut io: EngineStateAccess<'db, 'db, 'db>,
    env: env::Fixed,
    relayer_address: Address,
    promise_data: &[Option<Vec<u8>>],
) -> Result<Option<TransactionExecutionResult>, error::Error> {
    let result = match transaction {
        TransactionKind::Call(args) => {
            // We can ignore promises in the standalone engine (see above)
            let mut handler = crate::promise::NoScheduler { promise_data };
            let mut engine =
                engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;

            let result = engine.call_with_args(args.clone(), &mut handler);

            Some(TransactionExecutionResult::Submit(result))
        }

        TransactionKind::Deploy(input) => {
            // We can ignore promises in the standalone engine (see above)
            let mut handler = crate::promise::NoScheduler { promise_data };
            let mut engine =
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
            let mut engine =
                engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;

            if env.predecessor_account_id == env.current_account_id {
                connector::EthConnectorContract::init_instance(io)?
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

        TransactionKind::FtTransferCall(args) => {
            let input = if let Some(memo) = args.clone().memo {
                format!(
                    "{{\"sender_id\": {:?}, \"receiver_id\": {:?}, \"amount\": {:?}, \"memo\": {:?},  \"msg\": {:?} }}",
                    env.predecessor_account_id,
                    args.receiver_id.to_string(),
                    args.amount.to_string(),
                    memo,
                    args.msg
                )
            } else {
                format!(
                    "{{\"sender_id\": {:?}, \"receiver_id\": {:?}, \"amount\": {:?}, \"msg\": {:?} }}",
                    env.predecessor_account_id,
                    args.receiver_id.to_string(),
                    args.amount.to_string(),
                    args.msg
                )
            }.as_bytes().to_vec();
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            let promise_args = connector.ft_transfer_call(input);
            Some(TransactionExecutionResult::Promise(promise_args))
        }

        TransactionKind::FtTransfer(args) => {
            let connector = connector::EthConnectorContract::init_instance(io)?;
            let input = if let Some(memo) = args.clone(). memo {
                format!(
                    "{{\"sender_id\": {:?}, \"receiver_id\": {:?}, \"amount\": {:?}, \"memo\": {:?} }}",
                    env.predecessor_account_id,
                    args.receiver_id.to_string(),
                    args.amount.to_string(),
                    memo
                )
            } else {
                format!(
                    "{{\"sender_id\": {:?}, \"receiver_id\": {:?}, \"amount\": {:?} }}",
                    env.predecessor_account_id,
                    args.receiver_id.to_string(),
                    args.amount.to_string(),
                )
            }.as_bytes().to_vec();
            let promise_args = connector.ft_transfer(input);
            Some(TransactionExecutionResult::Promise(promise_args))
        }

        TransactionKind::Withdraw(args) => {
            use borsh::BorshSerialize;
            let input = EngineWithdrawCallArgs {
                sender_id: env.predecessor_account_id(),
                recipient_address: args.recipient_address,
                amount: args.amount,
            }
            .try_to_vec()
            .map_err(|_| error::Error::FtWithdraw(connector::error::WithdrawError::ParseArgs))?;

            let connector = connector::EthConnectorContract::init_instance(io)?;
            let promise_args = connector.withdraw_eth_from_near(input);
            Some(TransactionExecutionResult::Promise(promise_args))
        }

        TransactionKind::Deposit(args) => {
            let mut connector_contract = connector::EthConnectorContract::init_instance(io)?;
            connector_contract.set_eth_connector_contract_account(env.current_account_id);
            let promise_args = connector_contract.deposit(args.clone());
            Some(TransactionExecutionResult::Promise(promise_args))
        }

        TransactionKind::StorageDeposit(args) => {
            let connector = connector::EthConnectorContract::init_instance(io)?;

            let input = format!("\"sender_id\": {:?}", env.predecessor_account_id());
            let input = if let Some(account_id) = args.account_id.clone() {
                format!("{}, \"account_id\": {:?}", input, account_id.to_string())
            } else {
                input
            };
            let input = if let Some(registration_only) = args.registration_only {
                format!(
                    "{}, \"registration_only\": {:?}",
                    input,
                    registration_only.to_string()
                )
            } else {
                input
            };
            let input = format!("{{ {} }}", input).as_bytes().to_vec();

            let promise_args = connector.storage_deposit(input, env.attached_deposit);
            Some(TransactionExecutionResult::Promise(promise_args))
        }

        TransactionKind::StorageUnregister(force) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            let input = format!("\"sender_id\": {:?}", env.predecessor_account_id);
            let input = if let Some(force) = force {
                format!("{}, \"force\": {:?}", input, force)
            } else {
                input
            };
            let input = format!("{{ {} }}", input).as_bytes().to_vec();
            let promise_args = connector.storage_unregister(input);
            Some(TransactionExecutionResult::Promise(promise_args))
        }

        TransactionKind::StorageWithdraw(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            let input = format!("\"sender_id\": {:?}", env.predecessor_account_id);
            let input = if let Some(amount) = args.amount {
                format!("{}, \"amount\": {:?}", input, amount.as_u128())
            } else {
                input
            };
            let input = format!("{{ {} }}", input).as_bytes().to_vec();
            let promise_args = connector.storage_withdraw(input);
            Some(TransactionExecutionResult::Promise(promise_args))
        }

        TransactionKind::RegisterRelayer(evm_address) => {
            let mut engine =
                engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;
            engine.register_relayer(env.predecessor_account_id.as_bytes(), *evm_address);

            None
        }

        TransactionKind::RefundOnError(maybe_args) => {
            let result: Result<Option<TransactionExecutionResult>, engine::EngineStateError> =
                maybe_args
                    .clone()
                    .map(|args| {
                        let mut handler = crate::promise::NoScheduler { promise_data };
                        let engine_state = engine::get_state(&io)?;
                        let result =
                            engine::refund_on_error(io, &env, engine_state, args, &mut handler);
                        Ok(TransactionExecutionResult::Submit(result))
                    })
                    .transpose();

            result?
        }

        TransactionKind::SetConnectorData(args) => {
            let mut connector_io = io;
            connector::set_contract_data(&mut connector_io, args.clone())?;

            None
        }

        TransactionKind::NewConnector(_args) => None,
        TransactionKind::NewEngine(args) => {
            engine::set_state(&mut io, args.clone().into());

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
        TransactionKind::Unknown => None,
        // Not handled in this function; is handled by the general `execute_transaction` function
        TransactionKind::Submit(_) => unreachable!(),
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
    Promise(PromiseCreateArgs),
}

pub mod error {
    use aurora_engine::{connector, engine};

    #[derive(Debug)]
    pub enum Error {
        EngineState(engine::EngineStateError),
        Engine(engine::EngineError),
        DeployErc20(engine::DeployErc20Error),
        FtOnTransfer(connector::error::FtTransferCallError),
        Deposit(connector::error::DepositError),
        FinishDeposit(connector::error::FinishDepositError),
        FtTransfer(connector::error::TransferError),
        FtWithdraw(connector::error::WithdrawError),
        FtStorageFunding(connector::error::StorageFundingError),
        InvalidAddress(aurora_engine_types::types::address::error::AddressError),
        ConnectorInit(connector::error::InitContractError),
        ConnectorStorage(connector::error::StorageReadError),
    }

    impl From<engine::EngineStateError> for Error {
        fn from(e: engine::EngineStateError) -> Self {
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

    impl From<connector::error::FtTransferCallError> for Error {
        fn from(e: connector::error::FtTransferCallError) -> Self {
            Self::FtOnTransfer(e)
        }
    }

    impl From<connector::error::DepositError> for Error {
        fn from(e: connector::error::DepositError) -> Self {
            Self::Deposit(e)
        }
    }

    impl From<connector::error::FinishDepositError> for Error {
        fn from(e: connector::error::FinishDepositError) -> Self {
            Self::FinishDeposit(e)
        }
    }

    impl From<connector::error::TransferError> for Error {
        fn from(e: connector::error::TransferError) -> Self {
            Self::FtTransfer(e)
        }
    }

    impl From<connector::error::WithdrawError> for Error {
        fn from(e: connector::error::WithdrawError) -> Self {
            Self::FtWithdraw(e)
        }
    }

    impl From<connector::error::StorageFundingError> for Error {
        fn from(e: connector::error::StorageFundingError) -> Self {
            Self::FtStorageFunding(e)
        }
    }

    impl From<aurora_engine_types::types::address::error::AddressError> for Error {
        fn from(e: aurora_engine_types::types::address::error::AddressError) -> Self {
            Self::InvalidAddress(e)
        }
    }

    impl From<connector::error::InitContractError> for Error {
        fn from(e: connector::error::InitContractError) -> Self {
            Self::ConnectorInit(e)
        }
    }

    impl From<connector::error::StorageReadError> for Error {
        fn from(e: connector::error::StorageReadError) -> Self {
            Self::ConnectorStorage(e)
        }
    }
}

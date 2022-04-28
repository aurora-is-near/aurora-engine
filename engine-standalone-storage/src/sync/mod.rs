use aurora_engine::{connector, engine, parameters::SubmitResult};
use aurora_engine_sdk::env::{self, Env, DEFAULT_PREPAID_GAS};
use aurora_engine_types::{
    parameters::PromiseWithCallbackArgs,
    types::{Address, Yocto},
    H256,
};

pub mod types;

use crate::engine_state::EngineStateAccess;
use crate::{BlockMetadata, Diff, Storage};
use types::{Message, TransactionKind};

use self::types::TransactionMessage;

const AURORA_ACCOUNT_ID: &str = "aurora";

pub fn consume_message(
    storage: &mut Storage,
    message: Message,
) -> Result<ConsumeMessageOutcome, error::Error> {
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

            let (tx_hash, diff, result) = storage
                .with_engine_access(block_height, transaction_position, &[], |io| {
                    execute_transaction(
                        transaction_message.as_ref(),
                        block_height,
                        &block_metadata,
                        io,
                    )
                })
                .result?;
            match &result {
                Some(TransactionExecutionResult::Submit(Err(_))) => (), // do not persist if Engine encounters an error
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
) -> Result<TransactionIncludedOutcome, error::Error> {
    let transaction_position = transaction_message.position;
    let block_hash = transaction_message.block_hash;
    let block_height = storage.get_block_height_by_hash(block_hash)?;
    let block_metadata = storage.get_block_metadata(block_hash)?;
    let result = storage.with_engine_access(block_height, transaction_position, &[], |io| {
        execute_transaction(&transaction_message, block_height, &block_metadata, io)
    });
    let (tx_hash, diff, maybe_result) = result.result?;
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
    mut io: EngineStateAccess<'db, 'db, 'db>,
) -> Result<(H256, Diff, Option<TransactionExecutionResult>), error::Error> {
    let signer_account_id = transaction_message.signer.clone();
    let predecessor_account_id = transaction_message.caller.clone();
    let relayer_address =
        aurora_engine_sdk::types::near_account_to_evm_address(predecessor_account_id.as_bytes());
    let near_receipt_id = transaction_message.near_receipt_id;
    let current_account_id = AURORA_ACCOUNT_ID.parse().unwrap();
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
            let mut handler = crate::promise::Noop;
            let engine_state = engine::get_state(&io)?;
            let transaction_bytes: Vec<u8> = tx.into();
            let tx_hash = aurora_engine_sdk::keccak(&transaction_bytes);

            let result = engine::submit(
                io,
                &env,
                &transaction_bytes,
                engine_state,
                env.current_account_id(),
                relayer_address,
                &mut handler,
            );

            (tx_hash, Some(TransactionExecutionResult::Submit(result)))
        }

        TransactionKind::Call(args) => {
            // We can ignore promises in the standalone engine (see above)
            let mut handler = crate::promise::Noop;
            let mut engine =
                engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;

            let result = engine.call_with_args(args.clone(), &mut handler);

            (
                near_receipt_id,
                Some(TransactionExecutionResult::Submit(result)),
            )
        }

        TransactionKind::Deploy(input) => {
            // We can ignore promises in the standalone engine (see above)
            let mut handler = crate::promise::Noop;
            let mut engine =
                engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;

            let result = engine.deploy_code_with_input(input.clone(), &mut handler);

            (
                near_receipt_id,
                Some(TransactionExecutionResult::Submit(result)),
            )
        }

        TransactionKind::DeployErc20(args) => {
            // No promises can be created by `deploy_erc20_token`
            let mut handler = crate::promise::Noop;
            let result = engine::deploy_erc20_token(args.clone(), io, &env, &mut handler)?;
            (
                near_receipt_id,
                Some(TransactionExecutionResult::DeployErc20(result)),
            )
        }

        TransactionKind::FtOnTransfer(args) => {
            // No promises can be created by `ft_on_transfer`
            let mut handler = crate::promise::Noop;
            let mut engine =
                engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;

            if env.predecessor_account_id == env.current_account_id {
                connector::EthConnectorContract::init_instance(io).ft_on_transfer(&engine, args)?;
            } else {
                engine.receive_erc20_tokens(
                    &env.predecessor_account_id,
                    &env.signer_account_id,
                    args,
                    &env.current_account_id,
                    &mut handler,
                );
            }

            (near_receipt_id, None)
        }

        TransactionKind::FtTransferCall(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io);
            let promise_args = connector.ft_transfer_call(
                env.predecessor_account_id.clone(),
                env.current_account_id.clone(),
                args.clone(),
                env.prepaid_gas,
            )?;

            (
                near_receipt_id,
                Some(TransactionExecutionResult::Promise(promise_args)),
            )
        }

        TransactionKind::ResolveTransfer(args, promise_result) => {
            let mut connector = connector::EthConnectorContract::init_instance(io);
            connector.ft_resolve_transfer(args.clone(), promise_result.clone());

            (near_receipt_id, None)
        }

        TransactionKind::FtTransfer(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io);
            connector.ft_transfer(&env.predecessor_account_id, args.clone())?;

            (near_receipt_id, None)
        }

        TransactionKind::Withdraw(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io);
            connector.withdraw_eth_from_near(
                &env.current_account_id,
                &env.predecessor_account_id,
                args.clone(),
            )?;

            (near_receipt_id, None)
        }

        TransactionKind::Deposit(raw_proof) => {
            let connector_contract = connector::EthConnectorContract::init_instance(io);
            let promise_args = connector_contract.deposit(
                raw_proof.clone(),
                env.current_account_id(),
                env.predecessor_account_id(),
            )?;

            (
                near_receipt_id,
                Some(TransactionExecutionResult::Promise(promise_args)),
            )
        }

        TransactionKind::FinishDeposit(finish_args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io);
            let maybe_promise_args = connector.finish_deposit(
                env.predecessor_account_id(),
                env.current_account_id(),
                finish_args.clone(),
                env.prepaid_gas,
            )?;

            (
                near_receipt_id,
                maybe_promise_args.map(TransactionExecutionResult::Promise),
            )
        }

        TransactionKind::StorageDeposit(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io);
            let _ = connector.storage_deposit(
                env.predecessor_account_id,
                Yocto::new(env.attached_deposit),
                args.clone(),
            )?;

            (near_receipt_id, None)
        }

        TransactionKind::StorageUnregister(force) => {
            let mut connector = connector::EthConnectorContract::init_instance(io);
            let _ = connector.storage_unregister(env.predecessor_account_id, *force)?;

            (near_receipt_id, None)
        }

        TransactionKind::StorageWithdraw(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io);
            connector.storage_withdraw(&env.predecessor_account_id, args.clone())?;

            (near_receipt_id, None)
        }

        TransactionKind::SetPausedFlags(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io);
            connector.set_paused_flags(args.clone());

            (near_receipt_id, None)
        }

        TransactionKind::RegisterRelayer(evm_address) => {
            let mut engine =
                engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;
            engine.register_relayer(env.predecessor_account_id.as_bytes(), *evm_address);

            (near_receipt_id, None)
        }

        TransactionKind::RefundOnError(maybe_args) => {
            let result: Result<Option<TransactionExecutionResult>, engine::EngineStateError> =
                maybe_args
                    .clone()
                    .map(|args| {
                        let mut handler = crate::promise::Noop;
                        let engine_state = engine::get_state(&io)?;
                        let result =
                            engine::refund_on_error(io, &env, engine_state, args, &mut handler);
                        Ok(TransactionExecutionResult::Submit(result))
                    })
                    .transpose();

            (near_receipt_id, result?)
        }

        TransactionKind::SetConnectorData(args) => {
            let mut connector_io = io;
            connector::set_contract_data(&mut connector_io, args.clone())?;

            (near_receipt_id, None)
        }

        TransactionKind::NewConnector(args) => {
            connector::EthConnectorContract::create_contract(
                io,
                env.current_account_id,
                args.clone(),
            )?;

            (near_receipt_id, None)
        }
        TransactionKind::NewEngine(args) => {
            engine::set_state(&mut io, args.clone().into());

            (near_receipt_id, None)
        }
        TransactionKind::Unknown => (near_receipt_id, None),
    };

    let diff = io.get_transaction_diff();

    Ok((tx_hash, diff, result))
}

#[derive(Debug)]
pub enum ConsumeMessageOutcome {
    BlockAdded,
    FailedTransactionIgnored,
    TransactionIncluded(Box<TransactionIncludedOutcome>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionIncludedOutcome {
    pub hash: aurora_engine_types::H256,
    pub info: TransactionMessage,
    pub diff: crate::Diff,
    pub maybe_result: Option<TransactionExecutionResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionExecutionResult {
    Submit(engine::EngineResult<SubmitResult>),
    DeployErc20(Address),
    Promise(PromiseWithCallbackArgs),
}

pub mod error {
    use aurora_engine::{connector, engine, fungible_token};

    #[derive(Debug)]
    pub enum Error {
        Storage(crate::Error),
        EngineState(engine::EngineStateError),
        Engine(engine::EngineError),
        DeployErc20(engine::DeployErc20Error),
        FtOnTransfer(connector::error::FtTransferCallError),
        Deposit(connector::error::DepositError),
        FinishDeposit(connector::error::FinishDepositError),
        FtTransfer(fungible_token::error::TransferError),
        FtWithdraw(connector::error::WithdrawError),
        FtStorageFunding(fungible_token::error::StorageFundingError),
        InvalidAddress(aurora_engine_types::types::address::error::AddressError),
        ConnectorInit(connector::error::InitContractError),
    }

    impl From<crate::Error> for Error {
        fn from(e: crate::Error) -> Self {
            Self::Storage(e)
        }
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

    impl From<fungible_token::error::TransferError> for Error {
        fn from(e: fungible_token::error::TransferError) -> Self {
            Self::FtTransfer(e)
        }
    }

    impl From<connector::error::WithdrawError> for Error {
        fn from(e: connector::error::WithdrawError) -> Self {
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

    impl From<connector::error::InitContractError> for Error {
        fn from(e: connector::error::InitContractError) -> Self {
            Self::ConnectorInit(e)
        }
    }
}

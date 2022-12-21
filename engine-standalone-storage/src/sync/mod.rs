use aurora_engine::pausables::{
    EnginePrecompilesPauser, PausedPrecompilesManager, PrecompileFlags,
};
use aurora_engine::{connector, engine, parameters::SubmitResult, xcc};
use aurora_engine_sdk::env::{self, Env, DEFAULT_PREPAID_GAS};
use aurora_engine_types::{
    account_id::AccountId,
    parameters::PromiseWithCallbackArgs,
    types::{Address, Yocto},
    H256,
};

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
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            let promise_args = connector.ft_transfer_call(
                env.predecessor_account_id.clone(),
                env.current_account_id.clone(),
                args.clone(),
                env.prepaid_gas,
            )?;

            Some(TransactionExecutionResult::Promise(promise_args))
        }

        TransactionKind::ResolveTransfer(args, promise_result) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            connector.ft_resolve_transfer(args.clone(), promise_result.clone());

            None
        }

        TransactionKind::FtTransfer(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            connector.ft_transfer(&env.predecessor_account_id, args.clone())?;

            None
        }

        TransactionKind::Withdraw(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            connector.withdraw_eth_from_near(
                &env.current_account_id,
                &env.predecessor_account_id,
                args.clone(),
            )?;

            None
        }

        TransactionKind::Deposit(raw_proof) => {
            let connector_contract = connector::EthConnectorContract::init_instance(io)?;
            let promise_args = connector_contract.deposit(
                raw_proof.clone(),
                env.current_account_id(),
                env.predecessor_account_id(),
            )?;

            Some(TransactionExecutionResult::Promise(promise_args))
        }

        TransactionKind::FinishDeposit(finish_args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            let maybe_promise_args = connector.finish_deposit(
                env.predecessor_account_id(),
                env.current_account_id(),
                finish_args.clone(),
                env.prepaid_gas,
            )?;

            maybe_promise_args.map(TransactionExecutionResult::Promise)
        }

        TransactionKind::StorageDeposit(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            let _ = connector.storage_deposit(
                env.predecessor_account_id,
                Yocto::new(env.attached_deposit),
                args.clone(),
            )?;

            None
        }

        TransactionKind::StorageUnregister(force) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            let _ = connector.storage_unregister(env.predecessor_account_id, *force)?;

            None
        }

        TransactionKind::StorageWithdraw(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            connector.storage_withdraw(&env.predecessor_account_id, args.clone())?;

            None
        }

        TransactionKind::SetPausedFlags(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            connector.set_paused_flags(args.clone());

            None
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

        TransactionKind::NewConnector(args) => {
            connector::EthConnectorContract::create_contract(
                io,
                env.current_account_id,
                args.clone(),
            )?;

            None
        }
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
    Promise(PromiseWithCallbackArgs),
}

pub mod error {
    use aurora_engine::{connector, engine, fungible_token};

    #[derive(Debug)]
    pub enum Error {
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

    impl From<connector::error::StorageReadError> for Error {
        fn from(e: connector::error::StorageReadError) -> Self {
            Self::ConnectorStorage(e)
        }
    }
}

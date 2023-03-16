use aurora_engine::hashchain;
use aurora_engine::parameters::SubmitArgs;
use aurora_engine::pausables::{
    EnginePrecompilesPauser, PausedPrecompilesManager, PrecompileFlags,
};
use aurora_engine::{connector, engine, parameters::SubmitResult, state, xcc};
use aurora_engine_sdk::env::{self, Env, DEFAULT_PREPAID_GAS};
use aurora_engine_types::{
    account_id::AccountId,
    parameters::PromiseWithCallbackArgs,
    types::{Address, Yocto},
    H256,
};
use borsh::BorshSerialize;
use std::io;

pub mod types;

use crate::engine_state::EngineStateAccess;
use crate::{BlockMetadata, Diff, Storage};
use crate::sync::TransactionExecutionResult::{Submit, DeployErc20, Promise};
use types::{Message, TransactionKind, TransactionMessage};

use self::types::InnerTransactionKind;

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
            let tx_data: Vec<u8> = tx.into();
            let tx_hash = aurora_engine_sdk::keccak(&tx_data);
            let args = SubmitArgs {
                tx_data: tx_data.clone(),
                ..Default::default()
            };
            let result = state::get_state(&io)
                .map(|engine_state| {
                    let submit_result = engine::submit(
                        io,
                        &env,
                        &args,
                        engine_state,
                        env.current_account_id(),
                        relayer_address,
                        &mut handler,
                    );

                    let result = Some(TransactionExecutionResult::Submit(submit_result));
                    update_hashchain(io, env.block_height, &env.current_account_id, InnerTransactionKind::Submit, args.try_to_vec(), &result);
                    result
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
                    let submit_result = engine::submit(
                        io,
                        &env,
                        args,
                        engine_state,
                        env.current_account_id(),
                        relayer_address,
                        &mut handler,
                    );

                    let result = Some(TransactionExecutionResult::Submit(submit_result));
                    update_hashchain(io, env.block_height, &env.current_account_id, InnerTransactionKind::SubmitWithArgs, args.try_to_vec(), &result);
                    result
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
/// The `submit` transaction kind is special because it is the only one where the transaction hash
/// differs from the NEAR receipt hash.
#[allow(clippy::too_many_lines)]
fn non_submit_execute<'db>(
    transaction: &TransactionKind,
    mut io: EngineStateAccess<'db, 'db, 'db>,
    env: env::Fixed,
    relayer_address: Address,
    promise_data: &[Option<Vec<u8>>],
) -> Result<Option<TransactionExecutionResult>, error::Error> {
    let (input, result) = match transaction {
        TransactionKind::Call(args) => {
            // We can ignore promises in the standalone engine (see above)
            let mut handler = crate::promise::NoScheduler { promise_data };
            let mut engine =
                engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;

            let result = engine.call_with_args(args.clone(), &mut handler);

            (args.try_to_vec(), Some(TransactionExecutionResult::Submit(result)))
        }

        TransactionKind::Deploy(input) => {
            // We can ignore promises in the standalone engine (see above)
            let mut handler = crate::promise::NoScheduler { promise_data };
            let mut engine =
                engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;

            let result = engine.deploy_code_with_input(input.clone(), &mut handler);

            (Ok(input.to_vec()), Some(TransactionExecutionResult::Submit(result)))
        }

        TransactionKind::DeployErc20(args) => {
            // No promises can be created by `deploy_erc20_token`
            let mut handler = crate::promise::NoScheduler { promise_data };
            let result = engine::deploy_erc20_token(args.clone(), io, &env, &mut handler)?;

            (args.try_to_vec(), Some(TransactionExecutionResult::DeployErc20(result)))
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

            (args.try_to_vec(), None)
        }

        TransactionKind::FtTransferCall(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            let promise_args = connector.ft_transfer_call(
                env.predecessor_account_id.clone(),
                env.current_account_id.clone(),
                args.clone(),
                env.prepaid_gas,
            )?;

            (args.try_to_vec(), Some(TransactionExecutionResult::Promise(promise_args)))
        }

        TransactionKind::ResolveTransfer(args, promise_result) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            connector.ft_resolve_transfer(args, promise_result.clone());

            (args.try_to_vec(), None)
        }

        TransactionKind::FtTransfer(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            connector.ft_transfer(&env.predecessor_account_id, args)?;

            (args.try_to_vec(), None)
        }

        TransactionKind::Withdraw(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            connector.withdraw_eth_from_near(
                &env.current_account_id,
                &env.predecessor_account_id,
                args,
            )?;

            (args.try_to_vec(), None)
        }

        TransactionKind::Deposit(raw_proof) => {
            let connector_contract = connector::EthConnectorContract::init_instance(io)?;
            let promise_args = connector_contract.deposit(
                raw_proof.clone(),
                env.current_account_id(),
                env.predecessor_account_id(),
            )?;

            (Ok(raw_proof.to_vec()), Some(TransactionExecutionResult::Promise(promise_args)))
        }

        TransactionKind::FinishDeposit(finish_args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            let maybe_promise_args = connector.finish_deposit(
                env.predecessor_account_id(),
                env.current_account_id(),
                finish_args.clone(),
                env.prepaid_gas,
            )?;

            (finish_args.try_to_vec(), maybe_promise_args.map(TransactionExecutionResult::Promise))
        }

        TransactionKind::StorageDeposit(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            let _promise = connector.storage_deposit(
                env.predecessor_account_id,
                Yocto::new(env.attached_deposit),
                args.clone(),
            )?;

            (args.try_to_vec(), None)
        }

        TransactionKind::StorageUnregister(force) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            let _promise = connector.storage_unregister(env.predecessor_account_id, *force)?;

            (force.try_to_vec(), None)
        }

        TransactionKind::StorageWithdraw(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            connector.storage_withdraw(&env.predecessor_account_id, args)?;

            (args.try_to_vec(), None)
        }

        TransactionKind::SetPausedFlags(args) => {
            let mut connector = connector::EthConnectorContract::init_instance(io)?;
            connector.set_paused_flags(args);

            (args.try_to_vec(), None)
        }

        TransactionKind::RegisterRelayer(evm_address) => {
            let mut engine =
                engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;
            engine.register_relayer(env.predecessor_account_id.as_bytes(), *evm_address);

            (Ok(evm_address.as_bytes().to_vec()), None)
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

            (maybe_args.try_to_vec(), result?)
        }

        TransactionKind::SetConnectorData(args) => {
            let mut connector_io = io;
            connector::set_contract_data(&mut connector_io, args.clone())?;

            (args.try_to_vec(), None)
        }

        TransactionKind::NewConnector(args) => {
            connector::EthConnectorContract::create_contract(
                io,
                &env.current_account_id,
                args.clone(),
            )?;

            (args.try_to_vec(), None)
        }
        TransactionKind::NewEngine(args) => {
            state::set_state(&mut io, &args.clone().into())?;

            (args.try_to_vec(), None)
        }
        TransactionKind::FactoryUpdate(bytecode) => {
            let router_bytecode = xcc::RouterCode::borrowed(bytecode);
            xcc::update_router_code(&mut io, &router_bytecode);

            (Ok(bytecode.to_vec()), None)
        }
        TransactionKind::FactoryUpdateAddressVersion(args) => {
            xcc::set_code_version_of_address(&mut io, &args.address, args.version);

            (args.try_to_vec(), None)
        }
        TransactionKind::FactorySetWNearAddress(address) => {
            xcc::set_wnear_address(&mut io, address);

            (Ok(address.as_bytes().to_vec()), None)
        }
        TransactionKind::Unknown => (Ok(vec![]), None),
        // Not handled in this function; is handled by the general `execute_transaction` function
        TransactionKind::Submit(_) | TransactionKind::SubmitWithArgs(_) => unreachable!(),
        TransactionKind::PausePrecompiles(args) => {
            let precompiles_to_pause = PrecompileFlags::from_bits_truncate(args.paused_mask);

            let mut pauser = EnginePrecompilesPauser::from_io(io);
            pauser.pause_precompiles(precompiles_to_pause);

            (args.try_to_vec(), None)
        }
        TransactionKind::ResumePrecompiles(args) => {
            let precompiles_to_resume = PrecompileFlags::from_bits_truncate(args.paused_mask);

            let mut pauser = EnginePrecompilesPauser::from_io(io);
            pauser.resume_precompiles(precompiles_to_resume);

            (args.try_to_vec(), None)
        }
        TransactionKind::SetOwner(args) => {
            let mut prev = state::get_state(&io)?;

            prev.owner_id = args.clone().new_owner;
            state::set_state(&mut io, &prev)?;

            (args.try_to_vec(), None)
        }
    };

    update_hashchain(io, env.block_height, &env.current_account_id, transaction.into(), input, &result)?;

    Ok(result)
}

/// Updates the blockchain hashchain
fn update_hashchain<'db>(mut io: EngineStateAccess<'db, 'db, 'db>, block_height: u64, engine_account_id: &AccountId, inner_tx_type: InnerTransactionKind, input: Result<Vec<u8>, io::Error>, output: &Option<TransactionExecutionResult>) -> Result<(), error::Error> {
    let method_name = inner_tx_type.to_string();
    let input = input?;
    let output: Vec<u8> = match output {
        None => vec![],
        Some(t_e_r) => 
            match &t_e_r {
                Promise(_) => vec![],
                DeployErc20(address) => {address.as_bytes().to_vec()},
                Submit(engine_result) => {
                    match &engine_result {
                        Ok(submit_result) => submit_result.try_to_vec()?,
                        Err(e) => return Err(error::Error::Engine(e.clone())),
                    }
                }
            }
    };

    let mut blockchain_hashchain = hashchain::get_state(&io).unwrap_or_else(|_| {
        hashchain::BlockchainHashchain::new(
            &state::get_state(&io).unwrap().chain_id,
            engine_account_id.as_bytes(),
            block_height,
            [0; 32],
            [0; 32],
        )
    });

    if block_height > blockchain_hashchain.get_current_block_height() {
        blockchain_hashchain
            .move_to_block(block_height)?;
    }

    blockchain_hashchain
        .add_block_tx(block_height, &method_name, &input, &output)?;

    Ok(hashchain::set_state(&mut io, blockchain_hashchain)?)
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
    use aurora_engine::{connector, engine, fungible_token, state, hashchain::blockchain_hashchain_error::BlockchainHashchainError};
    use std::io;

    #[derive(Debug)]
    pub enum Error {
        EngineState(state::EngineStateError),
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
        BlockchainHashchain(BlockchainHashchainError),
        IO(io::Error),
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

    impl From<BlockchainHashchainError> for Error {
        fn from(e: BlockchainHashchainError) -> Self {
            Self::BlockchainHashchain(e)
        }
    }

    impl From<io::Error> for Error {
        fn from(e: io::Error) -> Self {
            Self::IO(e)
        }
    }
}

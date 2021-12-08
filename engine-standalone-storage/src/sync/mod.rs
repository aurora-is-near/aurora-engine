use aurora_engine::{connector, engine, parameters};
use aurora_engine_sdk::env::{self, Env};
use aurora_engine_types::TryFrom;
use borsh::BorshDeserialize;

pub mod types;

use types::{Message, TransactionKind};

const AURORA_ACCOUNT_ID: &str = "aurora";

pub fn consume_message(storage: &mut crate::Storage, message: Message) -> Result<(), error::Error> {
    match message {
        Message::Block(block_message) => {
            let block_hash = block_message.hash;
            let block_height = block_message.height;
            let block_metadata = block_message.metadata;
            storage
                .set_block_data(block_hash, block_height, block_metadata)
                .map_err(crate::Error::Rocksdb)?;
            Ok(())
        }

        Message::Transaction(transaction_message) => {
            // Failed transactions have no impact on the state of our database.
            if !transaction_message.succeeded {
                return Ok(());
            }

            let signer_account_id = transaction_message.signer;
            let predecessor_account_id = transaction_message.caller;
            let relayer_address = aurora_engine_sdk::types::near_account_to_evm_address(
                predecessor_account_id.as_bytes(),
            );
            let transaction_position = transaction_message.position;
            let near_tx_hash = transaction_message.near_tx_hash;
            let block_hash = transaction_message.block_hash;
            let block_height = storage.get_block_height_by_hash(block_hash)?;
            let block_metadata = storage.get_block_metadata(block_hash)?;
            let current_account_id = AURORA_ACCOUNT_ID.parse().unwrap();
            let env = env::Fixed {
                signer_account_id,
                current_account_id,
                predecessor_account_id,
                block_height,
                block_timestamp: block_metadata.timestamp,
                attached_deposit: transaction_message.attached_near,
                random_seed: block_metadata.random_seed,
            };
            let io =
                storage.access_engine_storage_at_position(block_height, transaction_position, &[]);

            let tx_hash = match transaction_message.transaction {
                TransactionKind::Submit(tx) => {
                    // Only promises possible from `submit` are exit precompiles and we cannot act on those promises
                    let mut handler = crate::promise::Noop;
                    let engine_state = engine::get_state(&io)?;
                    let transaction_bytes: Vec<u8> = tx.into();
                    let tx_hash = aurora_engine_sdk::keccak(&transaction_bytes);

                    let _result = engine::submit(
                        io,
                        &env,
                        &transaction_bytes,
                        engine_state,
                        env.current_account_id(),
                        relayer_address,
                        &mut handler,
                    )?;

                    tx_hash
                }

                TransactionKind::Call(args) => {
                    // Only promises possible from `call` are exit precompiles and we cannot act on those promises
                    let mut handler = crate::promise::Noop;
                    let mut engine =
                        engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;

                    let _result = engine.call_with_args(args, &mut handler)?;

                    near_tx_hash
                }

                TransactionKind::Deploy(input) => {
                    // Only promises possible from `deploy` are exit precompiles and we cannot act on those promises
                    let mut handler = crate::promise::Noop;
                    let mut engine =
                        engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;

                    let _result = engine.deploy_code_with_input(input, &mut handler)?;

                    near_tx_hash
                }

                TransactionKind::DeployErc20(args) => {
                    // No promises can be created by `deploy_erc20_token`
                    let mut handler = crate::promise::Noop;
                    let _result = engine::deploy_erc20_token(args, io, &env, &mut handler)?;
                    near_tx_hash
                }

                TransactionKind::FtOnTransfer(args) => {
                    // No promises can be created by `ft_on_transfer`
                    let mut handler = crate::promise::Noop;
                    let mut engine =
                        engine::Engine::new(relayer_address, env.current_account_id(), io, &env)?;

                    if env.predecessor_account_id == env.current_account_id {
                        connector::EthConnectorContract::init_instance(io)
                            .ft_on_transfer(&engine, &args)?;
                    } else {
                        engine.receive_erc20_tokens(
                            &env.predecessor_account_id,
                            &env.signer_account_id,
                            &args,
                            &env.current_account_id,
                            &mut handler,
                        );
                    }

                    near_tx_hash
                }

                TransactionKind::Deposit(raw_proof) => {
                    let mut connector_contract = connector::EthConnectorContract::init_instance(io);
                    let promise_args = connector_contract.deposit(
                        raw_proof,
                        env.current_account_id(),
                        env.predecessor_account_id(),
                    )?;

                    // Assume the relayer will mark `transaction.succeeded = false` if the
                    // proof failed to verify. This means the proof must be valid if we made
                    // it this far, so we will not worry about `promise_args.base` and move
                    // straight to the callback.

                    let finish_args = parameters::FinishDepositCallArgs::try_from_slice(
                        &promise_args.callback.args,
                    )
                    .expect("Connector deposit function must return valid args");
                    let maybe_promise_args = connector_contract.finish_deposit(
                        env.predecessor_account_id(),
                        env.current_account_id(),
                        finish_args,
                    )?;

                    if let Some(promise_args) = maybe_promise_args {
                        let on_transfer_args =
                            aurora_engine::json::parse_json(&promise_args.base.args)
                                .and_then(|json| {
                                    parameters::NEP141FtOnTransferArgs::try_from(json).ok()
                                })
                                .expect("Connector finish_deposit function must return valid args");
                        let engine = engine::Engine::new(
                            relayer_address,
                            env.current_account_id(),
                            io,
                            &env,
                        )?;
                        connector_contract.ft_on_transfer(&engine, &on_transfer_args)?;
                        // `ft_on_transfer` always returns an unused amount of 0 if it executes
                        // successfully, meaning that `ft_resolve_transfer` will do nothing,
                        // so we skip the promise_args callback.
                    }

                    near_tx_hash
                }
            };

            let diff = io.get_transaction_diff();
            let tx_included = crate::TransactionIncluded {
                block_hash,
                position: transaction_position,
            };
            storage.set_transaction_included(tx_hash, &tx_included, &diff)?;

            Ok(())
        }
    }
}

pub mod error {
    use aurora_engine::{connector, engine};

    #[derive(Debug)]
    pub enum Error {
        Storage(crate::Error),
        EngineState(engine::EngineStateError),
        Engine(engine::EngineError),
        DeployErc20(engine::DeployErc20Error),
        FtOnTransfer(connector::error::FtTransferCallError),
        Deposit(connector::error::DepositError),
        FinishDeposit(connector::error::FinishDepositError),
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
}

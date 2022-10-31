use aurora_engine::engine;
use aurora_engine_sdk::env::{self, Env, DEFAULT_PREPAID_GAS};
use aurora_engine_transactions::EthTransactionKind;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::H256;
use postgres::fallible_iterator::FallibleIterator;

use crate::{BlockMetadata, Storage};

pub mod types;

const TRANSACTION_QUERY: &str = "
SELECT
  transaction.block, transaction.index, transaction.id,
  transaction.hash, transaction.near_hash, transaction.near_receipt_hash,
  transaction.from, transaction.to, transaction.nonce, transaction.gas_price,
  transaction.gas_limit, transaction.gas_used, transaction.value, transaction.input,
  transaction.v, transaction.r, transaction.s, transaction.status, transaction.output,
  block.hash as block_hash
FROM transaction INNER JOIN block
ON transaction.block = block.id
ORDER BY transaction.block, transaction.index
";

/// Opens a Postgres connection to a running server hosting the relayer database.
pub fn connect_without_tls(
    connection_params: &types::ConnectionParams,
) -> Result<postgres::Client, postgres::Error> {
    let connection_string = connection_params.as_connection_string();
    postgres::Client::connect(&connection_string, postgres::NoTls)
}

pub fn read_block_data(
    connection: &mut postgres::Client,
) -> Result<postgres::RowIter<'_>, postgres::Error> {
    connection.query_raw::<_, u32, _>("SELECT * FROM block", std::iter::empty())
}

pub fn read_transaction_data(
    connection: &mut postgres::Client,
) -> Result<postgres::RowIter<'_>, postgres::Error> {
    connection.query_raw::<_, u32, _>(TRANSACTION_QUERY, std::iter::empty())
}

pub fn initialize_blocks<I>(storage: &mut Storage, mut rows: I) -> Result<(), error::Error>
where
    I: FallibleIterator<Item = types::BlockRow, Error = postgres::Error>,
{
    while let Some(row) = rows.next()? {
        let metadata = BlockMetadata {
            timestamp: env::Timestamp::new(row.timestamp.unwrap_or(0)),
            // TODO: need relayer to index this, tracking issue: https://github.com/aurora-is-near/aurora-relayer/issues/135
            random_seed: H256([0; 32]),
        };

        storage
            .set_block_data(row.hash, row.id, metadata)
            .map_err(crate::Error::Rocksdb)?;
    }
    Ok(())
}

pub fn initialize_transactions<I>(
    storage: &mut Storage,
    mut rows: I,
    engine_state: engine::EngineState,
) -> Result<(), error::Error>
where
    I: FallibleIterator<Item = types::TransactionRow, Error = postgres::Error>,
{
    let signer_account_id = "relayer.aurora".parse().unwrap();
    let predecessor_account_id: AccountId = "relayer.aurora".parse().unwrap();
    let current_account_id = "aurora".parse().unwrap();
    let relayer_address =
        aurora_engine_sdk::types::near_account_to_evm_address(predecessor_account_id.as_bytes());
    let mut env = env::Fixed {
        signer_account_id,
        current_account_id,
        predecessor_account_id,
        block_height: 0,
        block_timestamp: env::Timestamp::new(0),
        attached_deposit: 0,
        random_seed: H256::zero(),
        prepaid_gas: DEFAULT_PREPAID_GAS,
    };
    // We use the Noop handler here since the relayer DB does not contain any promise information.
    let mut handler = aurora_engine_sdk::promise::Noop;

    while let Some(row) = rows.next()? {
        let near_tx_hash = row.near_hash;
        let tx_succeeded = row.status;
        let transaction_position = row.index;
        let block_height = row.block;
        let block_hash = row.block_hash;
        let block_metadata = storage.get_block_metadata(block_hash)?;
        let tx: EthTransactionKind = row.into();
        let transaction_bytes: Vec<u8> = (&tx).into();
        let tx_hash = aurora_engine_sdk::keccak(&transaction_bytes);

        env.block_height = block_height;
        env.block_timestamp = block_metadata.timestamp;
        env.random_seed = block_metadata.random_seed;

        let result = storage.with_engine_access(block_height, transaction_position, &[], |io| {
            engine::submit(
                io,
                &env,
                &transaction_bytes,
                engine_state.clone(),
                env.current_account_id(),
                relayer_address,
                &mut handler,
            )
        });
        match result.result {
            // Engine errors would always turn into panics on the NEAR side, so we do not need to persist
            // any diff. Therefore, even if the error was expected, we still continue to the next transaction.
            Err(e) => {
                if tx_succeeded {
                    println!(
                        "WARN: Transaction with NEAR hash {:?} expected to succeed, but failed with error message {:?}",
                        near_tx_hash,
                        e
                    );
                }
                continue;
            }
            Ok(result) => {
                if result.status.is_fail() && tx_succeeded {
                    println!(
                        "WARN: Transaction with NEAR hash {:?} expected to succeed, but failed with error message {:?}",
                        near_tx_hash,
                        result.status
                    );
                    continue;
                }
                // if result.status.is_fail() && !tx_succeeded then this is consistent; we
                // should still persist the diff because failed transactions can impact the state.
                // For example, a transaction that runs of out of gas still has its balance deducted
                // for the gas spent. Therefore, we do not have a `continue` statement here.
            }
        }

        let diff = result.diff;
        let tx_msg = crate::TransactionMessage {
            block_hash,
            near_receipt_id: near_tx_hash,
            position: transaction_position,
            succeeded: true,
            signer: env.signer_account_id(),
            caller: env.predecessor_account_id(),
            attached_near: 0,
            transaction: crate::sync::types::TransactionKind::Submit(tx),
            promise_data: Vec::new(),
        };
        storage.set_transaction_included(tx_hash, &tx_msg, &diff)?;
    }
    Ok(())
}

pub mod error {
    use aurora_engine::engine;

    #[derive(Debug)]
    pub enum Error {
        Storage(crate::Error),
        Postgres(postgres::Error),
        EngineState(engine::EngineStateError),
        Engine(engine::EngineError),
    }

    impl From<crate::Error> for Error {
        fn from(e: crate::Error) -> Self {
            Self::Storage(e)
        }
    }

    impl From<postgres::Error> for Error {
        fn from(e: postgres::Error) -> Self {
            Self::Postgres(e)
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
}

#[cfg(test)]
mod test {
    use super::FallibleIterator;
    use crate::sync::types::{TransactionKind, TransactionMessage};
    use aurora_engine::{connector, engine, parameters};
    use aurora_engine_types::H256;

    /// Requires a running postgres server to work. A snapshot of the DB can be
    /// downloaded using the script from https://github.com/aurora-is-near/partner-relayer-deploy
    /// The postgres DB can be started in Docker using the following command:
    /// docker run --name mainnet_database -p '127.0.0.1:15432:5432' -v $PATH_TO_DB:/var/lib/postgresql/data auroraisnear/relayer-database:latest
    #[test]
    #[ignore]
    fn test_fill_db() {
        let mut storage = crate::Storage::open("rocks_tmp/").unwrap();
        let mut connection = super::connect_without_tls(&Default::default()).unwrap();
        let engine_state = engine::EngineState {
            chain_id: aurora_engine_types::types::u256_to_arr(&1313161555.into()),
            owner_id: "aurora".parse().unwrap(),
            bridge_prover_id: "prover.bridge.near".parse().unwrap(),
            upgrade_delay_blocks: 0,
        };

        // Initialize engine and connector states in storage.
        // Use explicit scope so borrows against `storage` are dropped before processing DB rows.
        {
            let block_height = 0;
            let block_hash = H256::zero();
            let block_metadata = crate::BlockMetadata {
                timestamp: aurora_engine_sdk::env::Timestamp::new(0),
                random_seed: H256::zero(),
            };
            storage
                .set_block_data(block_hash, block_height, block_metadata)
                .unwrap();
            let result = storage.with_engine_access(block_height, 0, &[], |io| {
                let mut local_io = io;
                engine::set_state(&mut local_io, engine_state.clone());
                connector::EthConnectorContract::create_contract(
                    io,
                    engine_state.owner_id.clone(),
                    parameters::InitCallArgs {
                        prover_account: engine_state.bridge_prover_id.clone(),
                        eth_custodian_address: "6bfad42cfc4efc96f529d786d643ff4a8b89fa52"
                            .to_string(),
                        metadata: Default::default(),
                    },
                )
            });

            result.result.ok().unwrap();
            let diff = result.diff;
            storage
                .set_transaction_included(
                    H256::zero(),
                    &TransactionMessage {
                        block_hash,
                        position: 0,
                        near_receipt_id: H256::zero(),
                        succeeded: true,
                        signer: "aurora".parse().unwrap(),
                        caller: "aurora".parse().unwrap(),
                        attached_near: 0,
                        transaction: TransactionKind::Unknown,
                        promise_data: Vec::new(),
                    },
                    &diff,
                )
                .unwrap();
        }
        let block_rows = super::read_block_data(&mut connection).unwrap();
        super::initialize_blocks(&mut storage, block_rows.map(|row| Ok(row.into()))).unwrap();
        let tx_rows = super::read_transaction_data(&mut connection).unwrap();
        super::initialize_transactions(
            &mut storage,
            tx_rows.map(|row| Ok(row.into())),
            engine_state,
        )
        .unwrap();

        connection.close().unwrap();
    }
}

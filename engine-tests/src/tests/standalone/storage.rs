use aurora_engine::engine;
use aurora_engine_sdk::env::Timestamp;
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{H256, U256};
use engine_standalone_storage::BlockMetadata;

use crate::test_utils::standalone::{mocks, storage::create_db};
use crate::test_utils::{self, Signer};

#[test]
fn test_replay_transaction() {
    let mut signer = Signer::random();
    let address = test_utils::address_from_secret_key(&signer.secret_key);
    let balance = Wei::new_u64(1000);
    let dest_address = test_utils::address_from_secret_key(&Signer::random().secret_key);
    let transfer_amounts: Vec<Wei> = vec![10, 13, 75, 88, 1, 9, 19, 256]
        .into_iter()
        .map(Wei::new_u64)
        .collect();
    let cumulative_transfer_amounts: Vec<Wei> = transfer_amounts
        .iter()
        .scan(Wei::zero(), |total, amount| {
            let new_total = *total + *amount;
            *total = new_total;
            Some(new_total)
        })
        .collect();
    let mut runner = test_utils::standalone::StandaloneRunner::default();
    let chain_id = Some(runner.chain_id);
    let create_transfer = |from: &mut Signer, to: Address, amount: Wei| {
        test_utils::sign_transaction(
            test_utils::transfer(to, amount, from.use_nonce().into()),
            chain_id,
            &from.secret_key,
        )
    };

    runner.init_evm();
    runner.mint_account(address, balance, signer.nonce.into(), None);

    let blockchain: mocks::block::Blockchain = vec![
        mocks::block::Block {
            height: 5,
            transactions: vec![
                create_transfer(&mut signer, dest_address, transfer_amounts[0]),
                create_transfer(&mut signer, dest_address, transfer_amounts[1]),
            ],
        },
        mocks::block::Block {
            height: 12,
            transactions: vec![
                create_transfer(&mut signer, dest_address, transfer_amounts[2]),
                create_transfer(&mut signer, dest_address, transfer_amounts[3]),
                create_transfer(&mut signer, dest_address, transfer_amounts[4]),
            ],
        },
        mocks::block::Block {
            height: 13,
            transactions: vec![create_transfer(
                &mut signer,
                dest_address,
                transfer_amounts[5],
            )],
        },
        mocks::block::Block {
            height: 20,
            transactions: vec![
                create_transfer(&mut signer, dest_address, transfer_amounts[6]),
                create_transfer(&mut signer, dest_address, transfer_amounts[7]),
            ],
        },
    ];

    // execute all the transactions
    let mut i = 0; // counter to keep track of which transaction we're on in the flattened list
    let sequential_diffs: Vec<Vec<_>> = blockchain
        .iter()
        .map(|block| {
            let block_height = block.height;
            block
                .transactions
                .iter()
                .enumerate()
                .map(|(position, tx)| {
                    let diff = runner
                        .execute_transaction_at_position(tx, block_height, position as u16)
                        .unwrap();

                    diff.clone()
                        .commit(&mut runner.storage, &mut runner.cumulative_diff);

                    assert_eq!(
                        runner.get_balance(&address),
                        balance - cumulative_transfer_amounts[i]
                    );
                    assert_eq!(
                        runner.get_balance(&dest_address),
                        cumulative_transfer_amounts[i]
                    );

                    i += 1;
                    diff
                })
                .collect()
        })
        .collect();

    // should be able to replay all transactions in any order
    let mut rng = rand::thread_rng();
    let mut shuffled: Vec<_> = blockchain.iter().zip(sequential_diffs).collect();
    rand::seq::SliceRandom::shuffle(shuffled.as_mut_slice(), &mut rng);

    for (block, diffs) in shuffled {
        let block_height = block.height;
        let mut txs: Vec<_> = block.transactions.iter().enumerate().zip(diffs).collect();
        rand::seq::SliceRandom::shuffle(txs.as_mut_slice(), &mut rng);
        for ((position, tx), diff) in txs {
            let replay_diff = runner
                .execute_transaction_at_position(tx, block_height, position as u16)
                .unwrap();
            assert_eq!(replay_diff, diff);
        }
    }
}

#[test]
fn test_consume_transaction() {
    // Some util structures we will use in this test
    let signer = Signer::random();
    let address = test_utils::address_from_secret_key(&signer.secret_key);
    let balance = Wei::new_u64(1000);
    let transfer_amount = Wei::new_u64(37);
    let nonce = signer.nonce.into();
    let dest_address = test_utils::address_from_secret_key(&Signer::random().secret_key);
    let mut runner = test_utils::standalone::StandaloneRunner::default();

    runner.init_evm();
    runner.mint_account(address, balance, nonce, None);

    // check pre-state
    assert_eq!(runner.get_balance(&address), balance);
    assert_eq!(runner.get_nonce(&address), U256::zero());

    // Try to execute a transfer transaction
    let tx = test_utils::transfer(dest_address, transfer_amount, nonce);
    let result = runner.submit_transaction(&signer.secret_key, tx).unwrap();
    assert!(result.status.is_ok());

    // Look at the engine state for the following block
    let engine_io =
        runner
            .storage
            .access_engine_storage_at_position(runner.env.block_height + 1, 0, &[]);

    // Confirm the balances and nonces match the expected values (note the transfer has been applied)
    assert_eq!(
        engine::get_balance(&engine_io, &address),
        balance - transfer_amount
    );
    assert_eq!(
        engine::get_balance(&engine_io, &dest_address),
        transfer_amount
    );
    assert_eq!(engine::get_nonce(&engine_io, &address), U256::one());
    assert_eq!(engine::get_nonce(&engine_io, &dest_address), U256::zero());

    runner.close();
}

#[test]
fn test_block_index() {
    let (temp_dir, mut storage) = create_db();

    let block_hash = H256([3u8; 32]);
    let block_height = 17u64;
    let block_metadata = BlockMetadata {
        timestamp: Timestamp::new(23_000),
        random_seed: H256([91u8; 32]),
    };

    // write block hash / height association
    storage
        .set_block_data(block_hash, block_height, block_metadata.clone())
        .unwrap();
    // read it back
    assert_eq!(
        block_hash,
        storage.get_block_hash_by_height(block_height).unwrap()
    );
    assert_eq!(
        block_height,
        storage.get_block_height_by_hash(block_hash).unwrap()
    );
    assert_eq!(
        block_metadata,
        storage.get_block_metadata(block_hash).unwrap()
    );

    // block hash / height that do not exist are errors
    let missing_block_height = block_height + 1;
    let missing_block_hash = H256([32u8; 32]);
    match storage.get_block_hash_by_height(missing_block_height) {
        Err(engine_standalone_storage::Error::NoBlockAtHeight(h)) if h == missing_block_height => {
            () // ok
        }
        other => panic!("Unexpected response: {:?}", other),
    }
    match storage.get_block_height_by_hash(missing_block_hash) {
        Err(engine_standalone_storage::Error::BlockNotFound(h)) if h == missing_block_hash => (), // ok
        other => panic!("Unexpected response: {:?}", other),
    }
    match storage.get_block_metadata(missing_block_hash) {
        Err(engine_standalone_storage::Error::BlockNotFound(h)) if h == missing_block_hash => (), // ok
        other => panic!("Unexpected response: {:?}", other),
    }

    drop(storage);
    temp_dir.close().unwrap();
}

#[test]
fn test_transaction_index() {
    let (temp_dir, mut storage) = create_db();

    let block_height = 37u64;
    mocks::insert_block(&mut storage, block_height);
    let block_hash = mocks::compute_block_hash(block_height);
    let tx_hash = H256([77u8; 32]);
    let tx_position = 0u16;
    let tx_included = engine_standalone_storage::TransactionIncluded {
        block_hash,
        position: tx_position,
    };
    let diff = {
        let mut tmp = engine_standalone_storage::Diff::default();
        let key = aurora_engine_types::storage::bytes_to_key(
            aurora_engine_types::storage::KeyPrefix::Balance,
            &[1u8; 20],
        );
        let value = crate::prelude::Wei::new_u64(159).to_bytes().to_vec();
        tmp.modify(key, value);
        tmp
    };

    // write transaction association
    storage
        .set_transaction_included(tx_hash, &tx_included, &diff)
        .unwrap();
    // read it back
    assert_eq!(
        tx_included,
        storage.get_transaction_by_hash(tx_hash).unwrap(),
    );
    assert_eq!(
        tx_hash,
        storage.get_transaction_by_position(tx_included).unwrap()
    );
    assert_eq!(
        diff.try_to_bytes().unwrap(),
        storage
            .get_transaction_diff(tx_included)
            .unwrap()
            .try_to_bytes()
            .unwrap()
    );

    // transactions that do not exist are errors
    let missing_block_hash = H256([32u8; 32]);
    let tx_not_included = engine_standalone_storage::TransactionIncluded {
        block_hash: missing_block_hash,
        position: 0,
    };
    let missing_tx_hash = H256([13u8; 32]);
    match storage.get_transaction_by_hash(missing_tx_hash) {
        Err(engine_standalone_storage::Error::TransactionHashNotFound(h))
            if h == missing_tx_hash =>
        {
            () // ok
        }
        other => panic!("Unexpected response: {:?}", other),
    }
    match storage.get_transaction_by_position(tx_not_included) {
        Err(engine_standalone_storage::Error::TransactionNotFound(x)) if x == tx_not_included => (), // ok
        other => panic!("Unexpected response: {:?}", other),
    }
    match storage.get_transaction_diff(tx_not_included) {
        Err(engine_standalone_storage::Error::TransactionNotFound(x)) if x == tx_not_included => (), // ok
        other => panic!("Unexpected response: {:?}", other),
    }

    drop(storage);
    temp_dir.close().unwrap();
}

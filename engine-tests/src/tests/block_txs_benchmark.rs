use crate::prelude::Address;
use crate::prelude::Wei;
use crate::test_utils::{
    self,
    erc20::{ERC20Constructor, ERC20},
    Signer,
};

use libsecp256k1::SecretKey;

const INITIAL_BALANCE: u64 = 1_000_000;
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: u64 = 10;

#[test]
fn block_txs_1() {
    block_txs_erc20_transfer_runner(255);
}

#[test]
fn block_txs_2() {
    block_txs_erc20_transfer_runner(1024);
}

fn block_txs_erc20_transfer_runner(txs_amount: usize) {
    let (mut runner, mut source_account, dest_address, contract) = initialize_erc20();
    let source_address = test_utils::address_from_secret_key(&source_account.secret_key);

    let initial_block_height = runner.context.block_index;

    let result = runner.submit_with_signer(&mut source_account, |nonce| {
        contract.mint(source_address, INITIAL_BALANCE.into(), nonce)
    });
    assert!(result.is_ok());
    assert_eq!(
        runner.context.block_index,
        initial_block_height + 1,
        "First tx; block has to be 1 more."
    );

    let mut block_txs_gas = Vec::with_capacity(txs_amount);

    for _ in 0..txs_amount {
        // transfer tx on block 2 (block index is increased interally before adding tx)
        let (result, profile) = runner
            .submit_with_signer_profiled(&mut source_account, |nonce| {
                contract.transfer(dest_address, TRANSFER_AMOUNT.into(), nonce)
            })
            .unwrap();

        assert!(result.status.is_ok());
        assert_eq!(
            runner.context.block_index,
            initial_block_height + 2,
            "Another tx, block has to be 2 more."
        );

        let tx_gas = profile.all_gas();
        block_txs_gas.push(tx_gas);

        runner.context.block_index -= 1;
        runner.context.block_timestamp -= 1_000_000_000;
    }

    println!("Experiment: Txs in block---------------------------------");
    println!("Txs amount:");
    println!("{:?}", txs_amount);
    println!("Block_txs_gas:");
    println!("{:?}", block_txs_gas);
}

#[test]
fn blocks_change_txs_1() {
    blocks_change_txs_erc20_transfer_runner(&[
        128 + 0,
        128 + 1,
        128 + 2,
        128 + 4,
        128 + 8,
        128 + 16,
        128 + 32,
        128 + 64,
        128 + 16 + 1,
        255,
    ]);
}

#[test]
fn blocks_change_txs_2() {
    blocks_change_txs_erc20_transfer_runner(&[129, 131, 133, 135, 137]);
}

fn blocks_change_txs_erc20_transfer_runner(blocks_txs_amounts: &[u32]) {
    let (mut runner, mut source_account, dest_address, contract) = initialize_erc20();
    let source_address = test_utils::address_from_secret_key(&source_account.secret_key);

    let mut expected_block_height = runner.context.block_index;

    let result = runner.submit_with_signer(&mut source_account, |nonce| {
        contract.mint(source_address, INITIAL_BALANCE.into(), nonce)
    });

    expected_block_height += 1;
    assert!(result.is_ok());
    assert_eq!(
        runner.context.block_index, expected_block_height,
        "Tx first; height should have move one."
    );

    let mut blocks_change_txs_gas = Vec::with_capacity(blocks_txs_amounts.len());

    // for each block, add txs and then change height to measure gas
    for block_txs_amount in blocks_txs_amounts {
        for _ in 0..*block_txs_amount {
            // transfer tx to the next block (block index is increased interally before adding tx)
            let result = runner
                .submit_with_signer(&mut source_account, |nonce| {
                    contract.transfer(dest_address, TRANSFER_AMOUNT.into(), nonce)
                })
                .unwrap();
            assert!(result.status.is_ok());

            // return to previous block so next tx lands on the same one
            runner.context.block_index -= 1;
            runner.context.block_timestamp -= 1_000_000_000;

            assert_eq!(
                runner.context.block_index, expected_block_height,
                "Tx added to block; height was reduced after so it should remain the same."
            );
        }

        assert_eq!(
            runner.context.block_index, expected_block_height,
            "Txs added to block; height should remain the same."
        );

        // move height so next tx goes to the next block and hashchain computation gets triggered
        expected_block_height += 1;
        runner.context.block_index += 1;
        runner.context.block_timestamp += 1_000_000_000;

        let (result, profile) = runner
            .submit_with_signer_profiled(&mut source_account, |nonce| {
                contract.transfer(dest_address, TRANSFER_AMOUNT.into(), nonce)
            })
            .unwrap();

        expected_block_height += 1;

        assert!(result.status.is_ok());
        assert_eq!(
            runner.context.block_index, expected_block_height,
            "Tx hashchain computation; height should have one more."
        );

        let block_change_tx_gas = profile.all_gas();
        blocks_change_txs_gas.push(block_change_tx_gas);
    }

    println!("Experiment: Txs in blocks change-------------------------");
    println!("Blocks txs amounts:");
    println!("{:?}", blocks_txs_amounts);
    println!("blocks_change_txs_gas:");
    println!("{:?}", blocks_change_txs_gas);
}

fn initialize_erc20() -> (test_utils::AuroraRunner, Signer, Address, ERC20) {
    // set up Aurora runner and accounts
    let mut runner = test_utils::deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    let source_address = test_utils::address_from_secret_key(&source_account);
    runner.create_address(
        source_address,
        Wei::new_u64(INITIAL_BALANCE),
        INITIAL_NONCE.into(),
    );
    let dest_address = test_utils::address_from_secret_key(&SecretKey::random(&mut rng));

    let mut signer = Signer::new(source_account);
    signer.nonce = INITIAL_NONCE;
    let nonce = signer.use_nonce();
    let constructor = ERC20Constructor::load();
    let contract = ERC20(runner.deploy_contract(
        &signer.secret_key,
        |c| c.deploy("TestToken", "TEST", nonce.into()),
        constructor,
    ));

    (runner, signer, dest_address, contract)
}

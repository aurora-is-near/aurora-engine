use crate::prelude::Wei;
use crate::prelude::{Address};
use crate::test_utils::{
    self,
    erc20::{ERC20Constructor, ERC20},
    Signer,
};

use libsecp256k1::SecretKey;

const INITIAL_BALANCE: u64 = 1_000_000;
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: u64 = 10;

const BLOCK_TRANSACTIONS_AMOUNT: u64 = 200;

#[test]
fn block_txs_erc20_transfer() {
    let (mut runner, mut source_account, dest_address, contract) = initialize_erc20(); 
    let source_address = test_utils::address_from_secret_key(&source_account.secret_key);

    let initial_block_height = runner.context.block_index;

    let result = runner.submit_with_signer(&mut source_account, |nonce| {
        contract.mint(source_address, INITIAL_BALANCE.into(), nonce)
    });
    assert!(result.is_ok());
    assert_eq!(runner.context.block_index, initial_block_height + 1, "First tx; block has to be 1 more.");

    let mut block_txs_gas: u64 = 0;

    for i in 0..BLOCK_TRANSACTIONS_AMOUNT {
        // transfer tx on block 2 (block index is increased interally before adding tx)
        let (result, profile) = runner
        .submit_with_signer_profiled(&mut source_account, |nonce| {
            contract.transfer(dest_address, TRANSFER_AMOUNT.into(), nonce)
        })
        .unwrap();

        assert!(result.status.is_ok());
        assert_eq!(runner.context.block_index, initial_block_height + 2, "Another tx, block has to be 2 more.");

        println!("Loop tx {:?}", i);

        block_txs_gas += profile.all_gas();
        runner.context.block_index -= 1;
        runner.context.block_timestamp -= 1_000_000_000;
    }

    assert_eq!(runner.context.block_index, initial_block_height + 1, "After loop, block has to be 1 more since we did a final decrease.");
    runner.context.block_index += 1;
    runner.context.block_timestamp += 1_000_000_000;

    // transfer tx on block 3 (block index is increased interally before adding tx)
    // this would trigger the block hashchain computation since there is a change on height
    let (result, profile) = runner
    .submit_with_signer_profiled(&mut source_account, |nonce| {
        contract.transfer(dest_address, TRANSFER_AMOUNT.into(), nonce)
    })
    .unwrap();

    assert!(result.status.is_ok());
    assert_eq!(runner.context.block_index, initial_block_height + 3, "After tx, block has to be 3 more.");

    let block_tx_hashchain_computation_gas = profile.all_gas();
    let block_total_gas = block_txs_gas + block_tx_hashchain_computation_gas;

    println!("block_txs_gas = {:?}", block_txs_gas);
    println!("block_tx_hashchain_computation_gas = {:?}", block_tx_hashchain_computation_gas);
    println!("block_total_gas = {:?}", block_total_gas);
    
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
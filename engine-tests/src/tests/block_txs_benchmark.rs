use crate::prelude::Wei;
use crate::prelude::{Address, U256};
use crate::test_utils::{
    self,
    erc20::{ERC20Constructor, ERC20},
    Signer,
};
use aurora_engine::parameters::TransactionStatus;
use aurora_engine_sdk as sdk;
use bstr::ByteSlice;
use libsecp256k1::SecretKey;

const INITIAL_BALANCE: u64 = 1_000_000;
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: u64 = 10;

const BLOCK_TRANSACTIONS_AMOUNT: U64 = 1_000;

#[test]
fn block_txs_erc20_transfer() {
    let (mut runner, mut source_account, dest_address, contract) = initialize_erc20();
    let source_address = test_utils::address_from_secret_key(&source_account.secret_key);

    let outcome = runner.submit_with_signer(&mut source_account, |nonce| {
        contract.mint(source_address, INITIAL_BALANCE.into(), nonce)
    });
    assert!(outcome.is_ok());

    let mut block_txs_total_gas: u64 = 0;

    for n in 0..BLOCK_TRANSACTIONS_AMOUNT {
        // Do transfer
        let outcome = runner
        .submit_with_signer_profiled(&mut source_account, |nonce| {
            contract.transfer(dest_address, TRANSFER_AMOUNT.into(), nonce)
        })
        .unwrap();
        assert!(outcome.status.is_ok());
    }
    
}

fn get_address_erc20_balance(
    runner: &mut test_utils::AuroraRunner,
    signer: &Signer,
    address: Address,
    contract: &ERC20,
) -> U256 {
    let balance_tx = contract.balance_of(address, signer.nonce.into());
    let result = runner
        .view_call(test_utils::as_view_call(
            balance_tx,
            test_utils::address_from_secret_key(&signer.secret_key),
        ))
        .unwrap();
    let bytes = match result {
        aurora_engine::parameters::TransactionStatus::Succeed(bytes) => bytes,
        err => panic!("Unexpected view call status {:?}", err),
    };
    U256::from_big_endian(&bytes)
}

fn parse_erc20_error_message(result: &[u8]) -> String {
    let start_index = result.find_char('&').unwrap();
    let end_index = result[start_index..].find_byte(0).unwrap() + start_index;

    String::from_utf8(result[start_index..end_index].to_vec()).unwrap()
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

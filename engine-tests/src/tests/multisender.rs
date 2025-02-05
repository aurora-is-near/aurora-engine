use crate::utils;
use crate::utils::solidity::erc20::ERC20;
use aurora_engine_transactions::legacy::TransactionLegacy;
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::U256;
use libsecp256k1::SecretKey;

const INITIAL_NONCE: u64 = 0;

#[test]
fn test_multisender_eth() {
    let (mut runner, mut signer, contract_address) = initialize();
    let mut multi_send_eth = |num_addr: usize| -> (u64, u64) {
        let destinations: Vec<(Address, U256)> = (0..num_addr)
            .map(|_| {
                let address = utils::address_from_secret_key(&utils::Signer::random().secret_key);
                let amount = Wei::from_eth(U256::one()).unwrap().raw();
                (address, amount)
            })
            .collect();

        let (result, profile) = runner
            .submit_with_signer_profiled(&mut signer, |nonce| {
                call_contract(contract_address, nonce, send_eth_data(&destinations))
            })
            .unwrap();
        utils::unwrap_success_slice(&result);

        (result.gas_used, profile.all_gas())
    };

    let (_evm_gas, near_gas) = multi_send_eth(350);

    assert!(near_gas / 1_000_000_000_000 < 300);
}

#[test]
fn test_multisender_erc20() {
    let (mut runner, mut signer, contract_address) = initialize();
    let signer_address = utils::address_from_secret_key(&signer.secret_key);

    let erc20 = {
        let constructor = utils::solidity::erc20::ERC20Constructor::load();
        let nonce = signer.use_nonce();
        let contract = runner.deploy_contract(
            &signer.secret_key,
            |c| c.deploy("TEST_A", "AAA", nonce.into()),
            constructor,
        );
        ERC20(contract)
    };
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            erc20.mint(signer_address, U256::from(u128::MAX), nonce)
        })
        .unwrap();
    utils::unwrap_success_slice(&result);

    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            erc20.approve(contract_address, U256::from(u128::MAX), nonce)
        })
        .unwrap();
    utils::unwrap_success_slice(&result);

    let mut multi_send_erc20 = |num_addr: usize| -> (u64, u64) {
        let destinations: Vec<(Address, U256)> = (0..num_addr)
            .map(|_| {
                let address = utils::address_from_secret_key(&utils::Signer::random().secret_key);
                let amount = U256::from(1);
                (address, amount)
            })
            .collect();

        let (result, profile) = runner
            .submit_with_signer_profiled(&mut signer, |nonce| {
                call_contract(
                    contract_address,
                    nonce,
                    send_erc20_data(erc20.0.address, &destinations),
                )
            })
            .unwrap();
        utils::unwrap_success_slice(&result);

        (result.gas_used, profile.all_gas())
    };

    let (_evm_gas, near_gas) = multi_send_erc20(150);

    assert!(near_gas / 1_000_000_000_000 < 300);
}

fn send_erc20_data(token_address: Address, amounts: &[(Address, U256)]) -> Vec<u8> {
    const SELECTOR: [u8; 4] = [142, 3, 28, 182];

    let amounts = convert_amounts(amounts);
    let tokens = vec![
        ethabi::Token::Address(token_address.raw().0.into()),
        ethabi::Token::Array(amounts),
    ];

    let mut result = Vec::new();
    result.extend_from_slice(&SELECTOR);
    result.extend_from_slice(&ethabi::encode(&tokens));

    result
}

fn send_eth_data(amounts: &[(Address, U256)]) -> Vec<u8> {
    const SELECTOR: [u8; 4] = [86, 232, 150, 19];

    let amounts = convert_amounts(amounts);
    let tokens = vec![ethabi::Token::Array(amounts)];

    let mut result = Vec::new();
    result.extend_from_slice(&SELECTOR);
    result.extend_from_slice(&ethabi::encode(&tokens));

    result
}

fn initialize() -> (utils::AuroraRunner, utils::Signer, Address) {
    let mut runner = utils::deploy_runner();
    runner.max_gas_burnt(u64::MAX);

    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    let source_address = utils::address_from_secret_key(&source_account);
    let initial_balance = Wei::new(U256::from(u128::MAX));
    runner.create_address(source_address, initial_balance, INITIAL_NONCE.into());
    let mut signer = utils::Signer::new(source_account);
    signer.nonce = INITIAL_NONCE;

    let deploy_code = hex::decode(
        std::fs::read_to_string("src/tests/res/multisender.hex")
            .unwrap()
            .trim(),
    )
    .unwrap();
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            let mut tx = utils::create_deploy_transaction(Vec::new(), nonce);
            tx.data = deploy_code;
            tx
        })
        .unwrap();
    let contract_address = Address::try_from_slice(utils::unwrap_success_slice(&result)).unwrap();

    let signer_address = utils::address_from_secret_key(&signer.secret_key);
    let result = runner
        .submit_with_signer(&mut signer, |nonce| {
            let mut tx = utils::transfer(contract_address, Wei::zero(), nonce);
            tx.data = initialize_data(signer_address);
            tx
        })
        .unwrap();
    utils::unwrap_success(result);

    (runner, signer, contract_address)
}

fn initialize_data(owner_address: Address) -> Vec<u8> {
    const SELECTOR: [u8; 4] = [196, 214, 109, 232];

    let mut result = Vec::new();

    let tokens = vec![ethabi::Token::Address(owner_address.raw().0.into())];
    result.extend_from_slice(&SELECTOR);
    result.extend_from_slice(&ethabi::encode(&tokens));

    result
}

fn call_contract(contract_address: Address, nonce: U256, data: Vec<u8>) -> TransactionLegacy {
    let mut tx = utils::transfer(contract_address, Wei::zero(), nonce);
    tx.data = data;
    tx
}

fn convert_amounts(amounts: &[(Address, U256)]) -> Vec<ethabi::Token> {
    amounts
        .iter()
        .map(|(addr, amount)| {
            ethabi::Token::Tuple(vec![
                ethabi::Token::Address(addr.raw().0.into()),
                ethabi::Token::Uint(amount.to_big_endian().into()),
            ])
        })
        .collect()
}

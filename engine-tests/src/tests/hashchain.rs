use aurora_engine::parameters::{StartHashchainArgs, SubmitResult, TransactionStatus};
use aurora_engine_hashchain::bloom::Bloom;
use aurora_engine_transactions::legacy::TransactionLegacy;
use aurora_engine_types::{
    H256, U256,
    types::{Address, Wei},
};

use crate::utils;

#[test]
fn test_hashchain() {
    let (mut runner, mut signer, _) = crate::tests::sanity::initialize_transfer();
    // Re-init the hashchain so we know the first tx is `start_hashchain`.
    let account_id = runner.aurora_account_id.clone();
    utils::init_hashchain(&mut runner, &account_id, None);

    // The tests initialize the hashchain with the default value.
    let hc = get_latest_hashchain(&runner);
    // Hashchain starts 1 height lower than the current context height because
    // at `hc.block_height + 1` the `start_hashchain` is submitted.
    assert_eq!(hc.block_height, runner.context.block_height - 1);
    assert_eq!(hc.hashchain, hex::encode(H256::default()));

    // Execute a transaction and the hashchain changes
    let transaction = TransactionLegacy {
        nonce: signer.use_nonce().into(),
        gas_price: U256::zero(),
        gas_limit: u64::MAX.into(),
        to: Some(Address::from_array([1u8; 20])),
        value: Wei::zero(),
        data: Vec::new(),
    };
    let signed_transaction =
        utils::sign_transaction(transaction, Some(runner.chain_id), &signer.secret_key);
    let input = rlp::encode(&signed_transaction).to_vec();
    let output = borsh::to_vec(&SubmitResult::new(
        TransactionStatus::Succeed(Vec::new()),
        21_000,
        Vec::new(),
    ))
    .unwrap();

    let expected_hc = {
        let start_hc_args = StartHashchainArgs {
            block_height: hc.block_height,
            block_hashchain: [0u8; 32],
        };
        let mut block_height = hc.block_height + 1;
        let mut hc = aurora_engine_hashchain::hashchain::Hashchain::new(
            aurora_engine_types::types::u256_to_arr(&runner.chain_id.into()),
            runner.aurora_account_id.parse().unwrap(),
            block_height,
            H256::default().0,
        );
        // First transaction is always `start_hashchain`
        hc.add_block_tx(
            block_height,
            "start_hashchain",
            &borsh::to_vec(&start_hc_args).unwrap(),
            &[],
            &Bloom::default(),
        )
        .unwrap();
        block_height += 1;
        hc.move_to_block(block_height).unwrap();
        // Insert the `submit` transaction we care about
        hc.add_block_tx(block_height, "submit", &input, &output, &Bloom::default())
            .unwrap();
        hc.move_to_block(block_height + 1).unwrap();
        hc.get_previous_block_hashchain()
    };

    runner
        .evm_submit(&signed_transaction, "relay.aurora")
        .unwrap();
    // Need to submit a second transaction to trigger hashchain computation on
    // the previous block (which contains the previous transaction)
    runner
        .submit_with_signer(&mut signer, |nonce| TransactionLegacy {
            nonce,
            gas_price: U256::zero(),
            gas_limit: u64::MAX.into(),
            to: None,
            value: Wei::zero(),
            data: Vec::new(),
        })
        .unwrap();

    let hc = get_latest_hashchain(&runner);
    assert_eq!(hc.block_height, runner.context.block_height - 1);
    assert_eq!(hc.hashchain, hex::encode(expected_hc));
}

fn get_latest_hashchain(runner: &utils::AuroraRunner) -> HashchainView {
    let outcome = runner
        .one_shot()
        .call("get_latest_hashchain", "any.near", Vec::new())
        .unwrap();
    let return_data = outcome.return_data.as_value().unwrap();
    let result: HashchainViewResult = serde_json::from_slice(&return_data).unwrap();
    result.result.unwrap()
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct HashchainViewResult {
    result: Option<HashchainView>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct HashchainView {
    block_height: u64,
    hashchain: String,
}

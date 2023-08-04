use crate::{
    bloom::Bloom,
    hashchain::{Hashchain, HashchainBuilder},
};
use aurora_engine_types::account_id::AccountId;

#[test]
fn test_add_tx() {
    let mut hashchain = HashchainBuilder::default()
        .with_current_block_height(2)
        .build();

    // Adding a transaction at a lower height than the current height is not allowed
    let add_tx_result = hashchain.add_block_tx(1, "foo", &[], &[], &Bloom::default());
    assert!(add_tx_result.is_err());
    assert!(hashchain.is_empty());
    assert_eq!(hashchain.get_logs_bloom(), &Bloom::default());

    // Adding a transaction at a higher height than the current height is not allowed
    let add_tx_result = hashchain.add_block_tx(3, "foo", &[], &[], &Bloom::default());
    assert!(add_tx_result.is_err());
    assert!(hashchain.is_empty());
    assert_eq!(hashchain.get_logs_bloom(), &Bloom::default());

    // Adding a transaction at the current height works
    let add_tx_result = hashchain.add_block_tx(2, "foo", &[], &[], &Bloom::default());
    assert!(add_tx_result.is_ok());
    assert!(!hashchain.is_empty());
    assert_eq!(hashchain.get_logs_bloom(), &Bloom::default());
}

#[test]
fn test_move_to_block_fail() {
    let mut hashchain = HashchainBuilder::default()
        .with_current_block_height(2)
        .build();

    // Cannot move to a height lower than the current height
    let move_to_block_result = hashchain.move_to_block(1);
    assert!(move_to_block_result.is_err());

    // Cannot move to the same height as the current height
    let move_to_block_result = hashchain.move_to_block(2);
    assert!(move_to_block_result.is_err());
}

#[test]
fn test_move_to_block_success() {
    let chain_id = [1; 32];
    let contract_account_id: AccountId = "aurora".parse().unwrap();
    let initial_hashchain = aurora_engine_sdk::keccak(b"seed");

    let method_name = "foo";
    let input = b"foo_input";
    let output = b"foo_output";
    let bloom = {
        let mut buf = Bloom::default();
        buf.0[0] = 1;
        buf
    };

    let tx_hash = aurora_engine_sdk::keccak(
        &[
            &len_be_bytes(method_name.as_bytes()),
            method_name.as_bytes(),
            &len_be_bytes(input),
            input,
            &len_be_bytes(output),
            output,
        ]
        .concat(),
    );

    let block_height_2: u64 = 2;
    let block_height_3 = block_height_2 + 1;
    let block_height_4 = block_height_3 + 1;

    let expected_hashchain_2 = aurora_engine_sdk::keccak(
        &[
            &chain_id,
            contract_account_id.as_bytes(),
            &block_height_2.to_be_bytes(),
            initial_hashchain.as_bytes(),
            tx_hash.as_bytes(),
            bloom.as_bytes(),
        ]
        .concat(),
    );

    let expected_hashchain_3 = aurora_engine_sdk::keccak(
        &[
            &chain_id,
            contract_account_id.as_bytes(),
            &block_height_3.to_be_bytes(),
            expected_hashchain_2.as_bytes(),
            &[0; 32],
            Bloom::default().as_bytes(),
        ]
        .concat(),
    );

    let expected_hashchain_4 = aurora_engine_sdk::keccak(
        &[
            &chain_id,
            contract_account_id.as_bytes(),
            &block_height_4.to_be_bytes(),
            expected_hashchain_3.as_bytes(),
            &[0; 32],
            Bloom::default().as_bytes(),
        ]
        .concat(),
    );

    let mut hashchain = HashchainBuilder::default()
        .with_account_id(contract_account_id)
        .with_chain_id(chain_id)
        .with_current_block_height(block_height_2)
        .with_previous_hashchain(initial_hashchain.0)
        .build();

    hashchain
        .add_block_tx(block_height_2, method_name, input, output, &bloom)
        .expect("Should add tx");
    assert_eq!(
        hashchain.get_previous_block_hashchain(),
        initial_hashchain.0
    );

    // Move to next height, capturing the hashchain that includes the previously added transaction
    hashchain
        .move_to_block(block_height_3)
        .expect("Should move to next block height");
    assert_eq!(
        hashchain.get_previous_block_hashchain(),
        expected_hashchain_2.0
    );
    assert!(
        hashchain.is_empty(),
        "Hashchain should be clear after moving to the next block"
    );
    assert_eq!(hashchain.get_logs_bloom(), &Bloom::default());

    // Should still work even if we skip a height
    hashchain
        .move_to_block(block_height_4 + 1)
        .expect("Should be able to skip a block height");
    assert_eq!(
        hashchain.get_previous_block_hashchain(),
        expected_hashchain_4.0
    );
    assert_eq!(hashchain.get_current_block_height(), block_height_4 + 1,);
}

#[test]
fn test_serialization_round_trip() {
    let bloom = {
        let mut bloom = Bloom::default();
        bloom.accrue(&[0xde, 0xad, 0xbe, 0xef]);
        bloom
    };

    let mut hashchain = HashchainBuilder::default()
        .with_account_id("aurora".parse().unwrap())
        .with_u64_chain_id(123456)
        .with_previous_hashchain([8; 32])
        .build();

    hashchain
        .add_block_tx(0, "foo", b"input", b"output", &bloom)
        .unwrap();

    let serialized = hashchain.try_serialize().unwrap();
    let round_trip = Hashchain::try_deserialize(&serialized).unwrap();

    assert_eq!(round_trip, hashchain);
}

fn len_be_bytes(arr: &[u8]) -> [u8; 4] {
    let len = arr.len();
    u32::try_from(len).unwrap().to_be_bytes()
}

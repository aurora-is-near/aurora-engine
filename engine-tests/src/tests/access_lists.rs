use crate::prelude::transactions::eip_2930::{self, AccessTuple, Transaction2930};
use crate::prelude::transactions::EthTransactionKind;
use crate::prelude::Wei;
use crate::prelude::{H256, U256};
use crate::test_utils;
use std::convert::TryFrom;
use std::iter;

// Test taken from https://github.com/ethereum/tests/blob/develop/GeneralStateTests/stExample/accessListExample.json
// TODO(#170): generally support Ethereum tests
#[test]
fn test_access_list_tx_encoding_decoding() {
    let secret_key = libsecp256k1::SecretKey::parse_slice(
        &hex::decode("45a915e4d060149eb4365960e6a7a45f334393093061116b197e3240065ff2d8").unwrap(),
    )
    .unwrap();
    let transaction = Transaction2930 {
        chain_id: 1,
        nonce: U256::zero(),
        gas_price: U256::from(0x0a),
        gas_limit: U256::from(0x061a80),
        to: Some(test_utils::address_from_hex(
            "0x095e7baea6a6c7c4c2dfeb977efac326af552d87",
        )),
        value: Wei::new_u64(0x0186a0),
        data: vec![0],
        access_list: vec![
            AccessTuple {
                address: test_utils::address_from_hex("0x095e7baea6a6c7c4c2dfeb977efac326af552d87")
                    .raw(),
                storage_keys: vec![H256::zero(), one()],
            },
            AccessTuple {
                address: test_utils::address_from_hex("0x195e7baea6a6c7c4c2dfeb977efac326af552d87")
                    .raw(),
                storage_keys: vec![H256::zero()],
            },
        ],
    };

    let signed_tx = test_utils::sign_access_list_transaction(transaction, &secret_key);
    let bytes: Vec<u8> = iter::once(eip_2930::TYPE_BYTE)
        .chain(rlp::encode(&signed_tx).into_iter())
        .collect();
    let expected_bytes = hex::decode("01f8f901800a83061a8094095e7baea6a6c7c4c2dfeb977efac326af552d87830186a000f893f85994095e7baea6a6c7c4c2dfeb977efac326af552d87f842a00000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000001f794195e7baea6a6c7c4c2dfeb977efac326af552d87e1a0000000000000000000000000000000000000000000000000000000000000000080a011c97e0bb8a356fe4f49b37863d059c6fe8cd3214a6ac06a8387a2f6f0b75f60a0212368a1097da30806edfd13d9c35662e1baee939235eb25de867980bd0eda26").unwrap();

    assert_eq!(bytes, expected_bytes);

    let decoded_tx = match EthTransactionKind::try_from(expected_bytes.as_slice()) {
        Ok(EthTransactionKind::Eip2930(tx)) => tx,
        Ok(_) => panic!("Unexpected transaction type"),
        Err(_) => panic!("Transaction parsing failed"),
    };

    assert_eq!(signed_tx, decoded_tx);

    assert_eq!(
        signed_tx.sender().unwrap(),
        test_utils::address_from_secret_key(&secret_key)
    )
}

fn one() -> H256 {
    let mut x = [0u8; 32];
    x[31] = 1;
    H256(x)
}

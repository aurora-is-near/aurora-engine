use borsh::BorshSerialize;

use near_crypto::{InMemorySigner, KeyType, PublicKey, Signature, Signer};

use aurora_engine::meta_parsing::{near_erc712_domain, parse_meta_call, prepare_meta_call_args};
use aurora_engine::parameters::MetaCallArgs;
use aurora_engine::prelude::{Address, U256};
use aurora_engine::types::{keccak, u256_to_arr, InternalMetaCallArgs};

pub fn encode_meta_call_function_args(
    signer: &dyn Signer,
    chain_id: u64,
    nonce: U256,
    fee_amount: U256,
    fee_address: Address,
    contract_address: Address,
    value: U256,
    method_def: &str,
    args: Vec<u8>,
) -> Vec<u8> {
    let domain_separator = near_erc712_domain(U256::from(chain_id));
    let (msg, _) = match prepare_meta_call_args(
        &domain_separator,
        "evm".as_bytes(),
        method_def.to_string(),
        &InternalMetaCallArgs {
            sender: Address::zero(),
            nonce,
            fee_amount,
            fee_address,
            contract_address,
            value,
            input: args.clone(),
        },
    ) {
        Ok(x) => x,
        Err(_) => panic!("Failed to prepare"),
    };
    match signer.sign(&msg) {
        Signature::ED25519(_) => panic!("Wrong Signer"),
        Signature::SECP256K1(sig) => {
            let array = Into::<[u8; 65]>::into(sig.clone()).to_vec();
            let mut signature = [0u8; 64];
            signature.copy_from_slice(&array[..64]);
            MetaCallArgs {
                signature,
                // Add 27 to align eth-sig-util signature format
                v: 27,
                nonce: u256_to_arr(&nonce),
                fee_amount: u256_to_arr(&fee_amount),
                fee_address: fee_address.0,
                contract_address: contract_address.0,
                value: u256_to_arr(&value),
                method_def: method_def.to_string(),
                args,
            }
            .try_to_vec()
            .expect("Failed to serialize")
        }
    }
}

pub fn public_key_to_address(public_key: PublicKey) -> Address {
    match public_key {
        PublicKey::ED25519(_) => panic!("Wrong PublicKey"),
        PublicKey::SECP256K1(pubkey) => {
            let pk: [u8; 64] = pubkey.into();
            let bytes = keccak(&pk.to_vec());
            let mut result = Address::zero();
            result.as_bytes_mut().copy_from_slice(&bytes[12..]);
            result
        }
    }
}

#[test]
fn test_meta_parsing() {
    let chain_id = 1313161555;
    let signer = InMemorySigner::from_seed("doesnt", KeyType::SECP256K1, "a");
    let signer_addr = public_key_to_address(signer.public_key.clone());
    let domain_separator = near_erc712_domain(U256::from(chain_id));

    let meta_tx = encode_meta_call_function_args(
        &signer,
        chain_id,
        U256::from(14),
        U256::from(6),
        Address::from_slice(&[0u8; 20]),
        signer_addr.clone(),
        U256::from(0),
        "adopt(uint256 petId)",
        // RLP encode of ["0x09"]
        hex::decode("c109").unwrap(),
    );

    // meta_tx[0..65] is eth-sig-util format signature
    // assert signature same as eth-sig-util, which also implies msg before sign (constructed by prepare_meta_call_args, follow eip-712) same
    assert_eq!(hex::encode(&meta_tx[0..65]), "4066a42cf17d167d33ef62c8cee82d3748de0e804569212a839257dafdbb9d09084bd910f16ddb9643e98a0787cdf0137cad109687a00106c701e430657ae99a1b");
    let result = parse_meta_call(&domain_separator, "evm".as_bytes(), meta_tx)
        .unwrap_or_else(|_| panic!("Fail meta_tx"));
    assert_eq!(result.sender, signer_addr);

    let meta_tx3 = encode_meta_call_function_args(
        &signer,
        chain_id,
        U256::from(14),
        U256::from(6),
        Address::from_slice(&[0u8; 20]),
        signer_addr.clone(),
        U256::from(0),
        "adopt(uint256 petId,PetObj petObject)PetObj(string petName,address owner)",
        // RLP encode of ["0x09", ["0x436170734C6F636B", "0x0123456789012345678901234567890123456789"]]
        hex::decode("e009de88436170734c6f636b940123456789012345678901234567890123456789").unwrap(),
    );
    assert_eq!(hex::encode(&meta_tx3[0..65]), "d5fc0804e27c7ee36178b5ce1f0ef97e9f9317855743f16a38cc2ec81eb852dc58f76aaebb8f0264eeb6a61ba5d094a546fa95efcded4d507708c1d96a3c06561b");
    let result = parse_meta_call(&domain_separator, "evm".as_bytes(), meta_tx3)
        .unwrap_or_else(|_| panic!("Fail meta_tx3"));

        assert_eq!(result.sender, signer_addr);
}

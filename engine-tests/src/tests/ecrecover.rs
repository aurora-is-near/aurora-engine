use super::sanity::initialize_transfer;
use crate::prelude::Wei;
use crate::prelude::{Address, U256};
use crate::test_utils::{self, AuroraRunner, Signer};
use aurora_engine_precompiles::Precompile;

const ECRECOVER_ADDRESS: Address = aurora_engine_precompiles::make_address(0, 1);

/// ecrecover tests taken from geth
#[test]
fn test_ecrecover_geth() {
    let (mut runner, mut signer, _) = initialize_transfer();

    let inputs = [
        "a8b53bdf3306a35a7103ab5504a0c9b492295564b6202b1942a84ef300107281000000000000000000000000000000000000000000000000000000000000001b307835653165303366353363653138623737326363623030393366663731663366353366356337356237346463623331613835616138623838393262346538621122334455667788991011121314151617181920212223242526272829303132",
        "18c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c000000000000000000000000000000000000000000000000000000000000001c73b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75feeb940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549",
        "18c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c100000000000000000000000000000000000000000000000000000000000001c73b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75feeb940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549",
        "18c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c000000000000000000000000000000000000001000000000000000000000001c73b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75feeb940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549",
        "18c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c000000000000000000000000000000000000001000000000000000000000011c73b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75feeb940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549",
    ];
    let outputs = [
        Vec::new(),
        hex::decode("000000000000000000000000a94f5374fce5edbc8e2a8697c15331677e6ebf0b").unwrap(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ];

    for (input, output) in inputs.iter().zip(outputs.iter()) {
        check_wasm_ecrecover(
            &mut runner,
            &mut signer,
            hex::decode(input).unwrap(),
            output,
        );
    }
}

/// The ecrecover implementation in both the standalone and wasm contract should be the same.
#[test]
fn test_ecrecover_standalone() {
    let (mut runner, mut signer, _) = initialize_transfer();

    let hash =
        hex::decode("5cc4cee58087de1a2ea481fe9c65c92adc27cff464b7f00a486dc9bf6bb8efb3").unwrap();
    let sig = hex::decode("32573a0b258f251971a4ec35511c018a7e7bf75a5886534b48d12e47263048a2fe6e03543955255e235388b224704555fd036a954d3ee6dd030d9d1fea1830d71c").unwrap();

    let input = construct_input(&hash, &sig);

    let ctx = evm::Context {
        address: Default::default(),
        caller: Default::default(),
        apparent_value: U256::zero(),
    };
    let standalone_result = aurora_engine_precompiles::secp256k1::ECRecover
        .run(&input, None, &ctx, false)
        .unwrap();

    check_wasm_ecrecover(&mut runner, &mut signer, input, &standalone_result.output);
}

fn check_wasm_ecrecover(
    runner: &mut AuroraRunner,
    signer: &mut Signer,
    input: Vec<u8>,
    expected_output: &[u8],
) {
    let wasm_result = runner
        .submit_with_signer(signer, |nonce| {
            aurora_engine_transactions::legacy::TransactionLegacy {
                nonce,
                gas_price: U256::zero(),
                gas_limit: u64::MAX.into(),
                to: Some(ECRECOVER_ADDRESS),
                value: Wei::zero(),
                data: input,
            }
        })
        .unwrap();
    assert_eq!(
        expected_output,
        test_utils::unwrap_success_slice(&wasm_result),
    );
}

fn construct_input(hash: &[u8], sig: &[u8]) -> Vec<u8> {
    let mut buf = [0u8; 128];
    (buf[0..32]).copy_from_slice(hash);
    buf[63] = sig[64];
    (buf[64..128]).copy_from_slice(&sig[0..64]);

    buf.to_vec()
}

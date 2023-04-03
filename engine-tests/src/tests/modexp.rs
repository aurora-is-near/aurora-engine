use super::sanity::initialize_transfer;
use crate::prelude::Wei;
use crate::prelude::{Address, U256};
use crate::test_utils::{self, AuroraRunner, Signer};

const MODEXP_ADDRESS: Address = aurora_engine_precompiles::make_address(0, 5);

#[test]
fn test_modexp_oom() {
    let (mut runner, mut signer, _) = initialize_transfer();

    let inputs = [
        // exp_len: i32::MAX
        "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007fffffff0000000000000000000000000000000000000000000000000000000000000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        // exp_len: u32::MAX
        "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ffffffff0000000000000000000000000000000000000000000000000000000000000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        // exp_len: u64::MAX
        "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000c000000000000000000000000000000000000000000000000000000000000000071000000000000ff600000000000000000000000000000000000000000000000",
    ];

    let outputs = [Vec::new(), Vec::new(), Vec::new()];

    for (input, output) in inputs.iter().zip(outputs.iter()) {
        check_wasm_modexp(
            &mut runner,
            &mut signer,
            hex::decode(input).unwrap(),
            output,
        );
    }
}

fn check_wasm_modexp(
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
                to: Some(MODEXP_ADDRESS),
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

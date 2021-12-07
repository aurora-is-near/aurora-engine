use aurora_engine_types::{types::wei::Wei, Address, U256};
use engine_standalone_tracing::sputnik;

use crate::test_utils::{self, standalone};

/// Test based on expected trace of
/// https://rinkeby.etherscan.io/tx/0xfc9359e49278b7ba99f59edac0e3de49956e46e530a53c15aa71226b7aa92c6f
/// (geth example found at https://gist.github.com/karalabe/c91f95ac57f5e57f8b950ec65ecc697f).
#[test]
fn test_evm_tracing() {
    let mut runner = standalone::StandaloneRunner::default();
    let mut signer = test_utils::Signer::random();

    // Initialize EVM
    runner.init_evm();

    // Deploy contract
    let deploy_tx = aurora_engine::transaction::legacy::TransactionLegacy {
        nonce: signer.use_nonce().into(),
        gas_price: U256::zero(),
        gas_limit: u64::MAX.into(),
        to: None,
        value: Wei::zero(),
        data: hex::decode(CONTRACT_CODE).unwrap(),
    };
    let result = runner
        .submit_transaction(&signer.secret_key, deploy_tx)
        .unwrap();
    let contract_address = Address::from_slice(test_utils::unwrap_success_slice(&result));

    // Interact with contract (and trace execution)
    let tx = aurora_engine::transaction::legacy::TransactionLegacy {
        nonce: signer.use_nonce().into(),
        gas_price: U256::zero(),
        gas_limit: 90_000.into(),
        to: Some(contract_address),
        value: Wei::zero(),
        data: hex::decode(CONTRACT_INPUT).unwrap(),
    };
    let mut listener = engine_standalone_tracing::sputnik::TransactionTraceBuilder::default();
    let result = sputnik::traced_call(&mut listener, || {
        runner.submit_transaction(&signer.secret_key, tx).unwrap()
    });
    assert!(result.status.is_ok());

    // Check trace
    let trace = listener.finish();
    let positions: Vec<u8> = trace
        .logs()
        .0
        .iter()
        .map(|l| l.program_counter.into_u32() as u8)
        .collect();
    assert_eq!(positions.as_slice(), &EXPECTED_POSITIONS);

    let costs: Vec<u32> = trace
        .logs()
        .0
        .iter()
        .map(|l| l.gas_cost.into_u64() as u32)
        .collect();
    assert_eq!(costs.as_slice(), &EXPECTED_COSTS);

    let op_codes: Vec<u8> = trace.logs().0.iter().map(|l| l.opcode.0).collect();
    assert_eq!(op_codes.as_slice(), &EXPECTED_OP_CODES);
}

const CONTRACT_CODE: &str = "60606040525b60008054600160a060020a03191633600160a060020a0316179055346001555b5b61011e806100356000396000f3006060604052361560465763ffffffff7c010000000000000000000000000000000000000000000000000000000060003504166383197ef08114604a5780638da5cb5b14605c575b5b5b005b3415605457600080fd5b60466095565b005b3415606657600080fd5b606c60d6565b60405173ffffffffffffffffffffffffffffffffffffffff909116815260200160405180910390f35b6000543373ffffffffffffffffffffffffffffffffffffffff9081169116141560d35760005473ffffffffffffffffffffffffffffffffffffffff16ff5b5b565b60005473ffffffffffffffffffffffffffffffffffffffff16815600a165627a7a7230582080eeb07bf95bf0cca20d03576cbb3a25de3bd0d1275c173d370dcc90ce23158d0029";
const CONTRACT_INPUT: &str = "2df07fbaabbe40e3244445af30759352e348ec8bebd4dd75467a9f29ec55d98d6cf6c418de0e922b1c55be39587364b88224451e7901d10a4a2ee2eeab3cccf51c";
const EXPECTED_POSITIONS: [u8; 27] = [
    0, 2, 4, 5, 6, 7, 9, 10, 15, 45, 47, 48, 49, 50, 55, 56, 57, 59, 60, 61, 66, 67, 69, 70, 71,
    72, 73,
];
const EXPECTED_COSTS: [u32; 27] = [
    3, 3, 12, 2, 3, 3, 10, 3, 3, 3, 3, 5, 3, 3, 3, 3, 3, 10, 3, 3, 3, 3, 10, 1, 1, 1, 0,
];
const EXPECTED_OP_CODES: [u8; 27] = [
    96, 96, 82, 54, 21, 96, 87, 99, 124, 96, 53, 4, 22, 99, 129, 20, 96, 87, 128, 99, 20, 96, 87,
    91, 91, 91, 0,
];

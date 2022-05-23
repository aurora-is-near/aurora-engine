use ::evm::backend::{MemoryAccount, MemoryBackend, MemoryVicinity};
use ::evm::executor::stack::{MemoryStackState, StackExecutor, StackSubstateMetadata};
use ::evm::{Config, Opcode};
use ethabi::ethereum_types::{H160, U256};
use std::{collections::BTreeMap, str::FromStr};
use std::convert::TryInto;
use secp256k1::sign;
use aurora_engine_transactions::legacy::TransactionLegacy;
use aurora_engine_types::types::{Address, Wei};
use engine_standalone_tracing::sputnik as evm;
use crate::test_utils;
use crate::test_utils::standalone;

#[test]
fn test_gas_is_not_miscalculated() {
    const GAS: usize = 10000;
    let input: [u8; 9] = [
        0x3a, 0x3a, 0x41, 0x59, 0x32, 0x32, 0x30, 0xf1, 0x84
    ];
    let config = Config::istanbul();
    let vicinity = MemoryVicinity {
        gas_price: U256::zero(),
        origin: H160::from_str("0x0000000000000000000000000000000000001234").unwrap(),
        block_hashes: Vec::new(),
        block_number: U256::from(9069000u64),
        block_coinbase: Default::default(),
        block_timestamp: Default::default(),
        block_difficulty: Default::default(),
        block_gas_limit: U256::from(GAS),
        chain_id: U256::one(),
        block_base_fee_per_gas: U256::zero(),
    };
    let mut state = BTreeMap::new();
    state.insert(
        H160::from_str("0x000000000000000000000000636f6e7472616374").unwrap(),
        MemoryAccount {
            nonce: U256::zero(),
            balance: U256::zero(),
            storage: BTreeMap::new(),
            code: input.to_vec(),
        }
    );
    state.insert(
        H160::from_str("0x0000000000000000000000000000000000001234").unwrap(),
        MemoryAccount {
            nonce: U256::zero(),
            balance: U256::zero(),
            storage: BTreeMap::new(),
            code: Vec::new(),
        },
    );
    let backend = MemoryBackend::new(&vicinity, state);
    let metadata = StackSubstateMetadata::new(u64::MAX, &config);
    let state = MemoryStackState::new(metadata, &backend);
    let precompiles = BTreeMap::new();
    let mut executor = StackExecutor::new_with_precompiles(state, &config, &precompiles);

    let mut runner = standalone::StandaloneRunner::default();
    let mut signer = test_utils::Signer::random();

    // Initialize EVM
    runner.init_evm();
    let mut listener = evm::TransactionTraceBuilder::default();
    let result = evm::traced_call(&mut listener, || {
        let caller = Address::new(H160::from_str("0x000000000000000000000000636f6e7472616374").unwrap());
        let signature = signer.secret_key;
        runner.submit_transaction(
            &signature,
            TransactionLegacy {
                nonce: U256::zero(),
                gas_price: U256::zero(),
                gas_limit: GAS.try_into().unwrap(),
                to: Some(Address::new(H160::from_str("0x000000000000000000000000636f6e7472616374").unwrap())),
                value: Wei::new(U256::zero()),
                data: input.to_vec()
            }
        ).unwrap()
    });
    assert!(result.status.is_ok());
    assert_eq!(result.gas_used, 45_038);
}

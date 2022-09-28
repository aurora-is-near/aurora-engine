use aurora_engine::engine;
use aurora_engine_sdk::env::DEFAULT_PREPAID_GAS;
use aurora_engine_test_doubles::io::{Storage, StoragePointer};
use aurora_engine_test_doubles::promise::PromiseTracker;
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{account_id::AccountId, H160, H256, U256};
use std::sync::RwLock;

#[test]
fn test_deploy_code() {
    let chain_id: [u8; 32] = {
        let value = U256::from(1313161554);
        let mut buf = [0u8; 32];
        value.to_big_endian(&mut buf);
        buf
    };
    let owner_id: AccountId = "aurora".parse().unwrap();
    let state = engine::EngineState {
        chain_id,
        owner_id: owner_id.clone(),
        bridge_prover_id: "mr_the_prover".parse().unwrap(),
        upgrade_delay_blocks: 0,
    };
    let origin = Address::new(H160([0u8; 20]));
    let storage = RwLock::new(Storage::default());
    let io = StoragePointer(&storage);
    let env = aurora_engine_sdk::env::Fixed {
        signer_account_id: owner_id.clone(),
        current_account_id: owner_id.clone(),
        predecessor_account_id: owner_id.clone(),
        block_height: 0,
        block_timestamp: aurora_engine_sdk::env::Timestamp::new(0),
        attached_deposit: 0,
        random_seed: H256::zero(),
        prepaid_gas: DEFAULT_PREPAID_GAS,
    };
    let mut handler = PromiseTracker::default();
    let mut engine = engine::Engine::new_with_state(state, origin, owner_id, io, &env);
    let code_to_deploy = vec![1, 2, 3, 4, 5, 6];
    let result = engine.deploy_code(
        origin,
        Wei::zero(),
        evm_deploy(&code_to_deploy),
        u64::MAX,
        Vec::new(),
        &mut handler,
    );

    // no promises are scheduled
    assert!(handler.scheduled_promises.is_empty());

    // execution was successful
    let contract_address = match result.unwrap().status {
        aurora_engine::parameters::TransactionStatus::Succeed(bytes) => {
            Address::try_from_slice(&bytes).unwrap()
        }
        other => panic!("Unexpected status: {:?}", other),
    };

    // state is updated
    assert_eq!(engine::get_balance(&io, &origin), Wei::zero());
    assert_eq!(engine::get_balance(&io, &contract_address), Wei::zero());
    assert_eq!(engine::get_nonce(&io, &origin), U256::one());
    assert_eq!(engine::get_nonce(&io, &contract_address), U256::one());
    assert_eq!(engine::get_generation(&io, &contract_address), 1);
    assert_eq!(engine::get_code(&io, &contract_address), code_to_deploy);
}

fn evm_deploy(code: &[u8]) -> Vec<u8> {
    let len = code.len();
    if len > u16::MAX as usize {
        panic!("Cannot deploy a contract with that many bytes!");
    }
    let len = len as u16;
    // This bit of EVM byte code essentially says:
    // "If msg.value > 0 revert; otherwise return `len` amount of bytes that come after me
    // in the code." By prepending this to `code` we create a valid EVM program which
    // returns `code`, which is exactly what we want.
    let init_code = format!(
        "608060405234801561001057600080fd5b5061{}806100206000396000f300",
        hex::encode(len.to_be_bytes())
    );
    hex::decode(init_code)
        .unwrap()
        .into_iter()
        .chain(code.iter().copied())
        .collect()
}

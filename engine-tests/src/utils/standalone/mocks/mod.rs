use aurora_engine::engine;
use aurora_engine::engine::Engine;
use aurora_engine_sdk::env::{DEFAULT_PREPAID_GAS, Env};
use aurora_engine_sdk::io::IO;
use aurora_engine_types::types::{Address, NearGas, Wei};
use aurora_engine_types::{H256, U256, account_id::AccountId};
use engine_standalone_storage::{BlockMetadata, Storage};

use crate::utils;

pub mod block;

pub const EXT_ETH_CONNECTOR: &str = "aurora_eth_connector.root";

pub fn compute_block_hash(block_height: u64) -> H256 {
    engine::compute_block_hash([0u8; 32], block_height, b"aurora")
}

pub fn insert_block(storage: &mut Storage, block_height: u64) {
    let block_hash = compute_block_hash(block_height);
    let block_metadata = BlockMetadata {
        timestamp: aurora_engine_sdk::env::Timestamp::new(0),
        random_seed: H256::zero(),
    };
    storage
        .set_block_data(block_hash, block_height, &block_metadata)
        .unwrap();
}

pub fn default_env(block_height: u64) -> aurora_engine_sdk::env::Fixed {
    let aurora_id: AccountId = utils::DEFAULT_AURORA_ACCOUNT_ID.parse().unwrap();
    aurora_engine_sdk::env::Fixed {
        signer_account_id: aurora_id.clone(),
        current_account_id: aurora_id.clone(),
        predecessor_account_id: aurora_id,
        block_height,
        block_timestamp: aurora_engine_sdk::env::Timestamp::new(0),
        attached_deposit: 0,
        random_seed: H256::zero(),
        prepaid_gas: DEFAULT_PREPAID_GAS,
        used_gas: NearGas::new(0),
    }
}

pub fn init_evm<I: IO + Copy, E: Env>(mut io: I, env: &E, chain_id: u64) {
    use aurora_engine_types::parameters::engine::NewCallArgsV2;

    let new_args = NewCallArgsV2 {
        chain_id: aurora_engine_types::types::u256_to_arr(&U256::from(chain_id)),
        owner_id: env.current_account_id(),
        upgrade_delay_blocks: 1,
    };

    aurora_engine::state::set_state(&mut io, &new_args.into()).unwrap();
}

pub fn init_connector<I: IO + Copy>(io: I) {
    use aurora_engine::contract_methods::connector::{
        set_connector_account_id, set_connector_withdraw_serialization_type,
    };
    use aurora_engine_types::parameters::connector::WithdrawSerializeType;

    set_connector_account_id(io, &EXT_ETH_CONNECTOR.parse().unwrap());
    set_connector_withdraw_serialization_type(io, &WithdrawSerializeType::Borsh);
}

pub fn mint_evm_account<I: IO + Copy, E: Env>(
    address: Address,
    balance: Wei,
    nonce: U256,
    code: Option<Vec<u8>>,
    io: I,
    env: &E,
) {
    use aurora_evm::backend::ApplyBackend;

    let mut engine: Engine<_, _> = Engine::new(address, env.current_account_id(), io, env).unwrap();
    let state_change = aurora_evm::backend::Apply::Modify {
        address: address.raw(),
        basic: aurora_evm::backend::Basic {
            balance: balance.raw(),
            nonce,
        },
        code,
        storage: std::iter::empty(),
        reset_storage: false,
    };

    engine.apply(std::iter::once(state_change), std::iter::empty(), false);
}

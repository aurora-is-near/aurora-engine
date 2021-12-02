use aurora_engine::engine;
use aurora_engine::fungible_token::FungibleTokenMetadata;
use aurora_engine::parameters::{FinishDepositCallArgs, InitCallArgs, NewCallArgs};
use aurora_engine_sdk::env::Env;
use aurora_engine_sdk::io::IO;
use aurora_engine_types::{account_id::AccountId, types::Wei, Address, H256, U256};
use engine_standalone_storage::Storage;

use crate::test_utils;

pub mod block;
pub mod promise;
pub mod storage;
pub mod tracing;

pub fn compute_block_hash(block_height: u64) -> H256 {
    aurora_engine::engine::compute_block_hash([0u8; 32], block_height, b"aurora")
}

pub fn insert_block(storage: &mut Storage, block_height: u64) {
    let block_hash = compute_block_hash(block_height);
    storage
        .set_block_hash_for_height(block_hash, block_height)
        .unwrap();
}

pub fn default_env(block_height: u64) -> aurora_engine_sdk::env::Fixed {
    let aurora_id: AccountId = test_utils::AuroraRunner::default()
        .aurora_account_id
        .parse()
        .unwrap();
    aurora_engine_sdk::env::Fixed {
        signer_account_id: aurora_id.clone(),
        current_account_id: aurora_id.clone(),
        predecessor_account_id: aurora_id,
        block_height,
        block_timestamp: aurora_engine_sdk::env::Timestamp::new(0),
        attached_deposit: 0,
        random_seed: H256::zero(),
    }
}

pub fn init_evm<I: IO + Copy, E: Env>(mut io: I, env: &E) {
    let chain_id = test_utils::AuroraRunner::default().chain_id;
    let new_args = NewCallArgs {
        chain_id: aurora_engine_types::types::u256_to_arr(&U256::from(chain_id)),
        owner_id: env.current_account_id(),
        bridge_prover_id: test_utils::str_to_account_id("bridge_prover.near"),
        upgrade_delay_blocks: 1,
    };

    engine::set_state(&mut io, new_args.into());

    let connector_args = InitCallArgs {
        prover_account: test_utils::str_to_account_id("prover.near"),
        eth_custodian_address: "d045f7e19B2488924B97F9c145b5E51D0D895A65".to_string(),
        metadata: FungibleTokenMetadata::default(),
    };

    aurora_engine::connector::EthConnectorContract::init_contract(
        io,
        env.current_account_id(),
        connector_args,
    )
    .map_err(unsafe_to_string)
    .unwrap();
}

pub fn mint_evm_account<I: IO + Copy, E: Env>(
    address: Address,
    balance: Wei,
    nonce: U256,
    code: Option<Vec<u8>>,
    mut io: I,
    env: &E,
) {
    use evm::backend::ApplyBackend;

    let aurora_account_id = env.current_account_id();
    let mut engine = engine::Engine::new(address, aurora_account_id.clone(), io, env).unwrap();
    let state_change = evm::backend::Apply::Modify {
        address,
        basic: evm::backend::Basic {
            balance: balance.raw(),
            nonce,
        },
        code,
        storage: std::iter::empty(),
        reset_storage: false,
    };
    engine.apply(std::iter::once(state_change), std::iter::empty(), false);

    let deposit_args = FinishDepositCallArgs {
        new_owner_id: aurora_account_id.clone(),
        amount: balance.raw().low_u128(),
        proof_key: String::new(),
        relayer_id: aurora_account_id.clone(),
        fee: 0,
        msg: None,
    };

    // Delete the fake proof so that we can use it again.
    let proof_key = crate::prelude::storage::bytes_to_key(
        crate::prelude::storage::KeyPrefix::EthConnector,
        &[crate::prelude::storage::EthConnectorStorageId::UsedEvent as u8],
    );
    io.remove_storage(&proof_key);

    aurora_engine::connector::EthConnectorContract::get_instance(io)
        .finish_deposit(
            aurora_account_id.clone(),
            aurora_account_id.clone(),
            deposit_args,
        )
        .map_err(unsafe_to_string)
        .unwrap();
}

pub fn unsafe_to_string<E: AsRef<[u8]>>(e: E) -> String {
    String::from_utf8(e.as_ref().to_vec()).unwrap()
}

use crate::test_utils;
use aurora_engine::engine;
use aurora_engine::fungible_token::FungibleTokenMetadata;
use aurora_engine::parameters::{
    FinishDepositCallArgs, InitCallArgs, NEP141FtOnTransferArgs, NewCallArgs,
};
use aurora_engine_sdk::env::{Env, DEFAULT_PREPAID_GAS};
use aurora_engine_sdk::io::IO;
use aurora_engine_types::types::{Address, Balance, NEP141Wei, NearGas, Wei};
use aurora_engine_types::{account_id::AccountId, H256, U256};
use engine_standalone_storage::{BlockMetadata, Storage};
use near_sdk_sim::DEFAULT_GAS;

pub mod block;

pub const ETH_CUSTODIAN_ADDRESS: Address =
    aurora_engine_precompiles::make_address(0xd045f7e1, 0x9b2488924b97f9c145b5e51d0d895a65);

pub fn compute_block_hash(block_height: u64) -> H256 {
    aurora_engine::engine::compute_block_hash([0u8; 32], block_height, b"aurora")
}

pub fn insert_block(storage: &mut Storage, block_height: u64) {
    let block_hash = compute_block_hash(block_height);
    let block_metadata = BlockMetadata {
        timestamp: aurora_engine_sdk::env::Timestamp::new(0),
        random_seed: H256::zero(),
    };
    storage
        .set_block_data(block_hash, block_height, block_metadata)
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
        prepaid_gas: DEFAULT_PREPAID_GAS,
    }
}

pub fn init_evm<I: IO + Copy, E: Env>(mut io: I, env: &E, chain_id: u64) {
    let new_args = NewCallArgs {
        chain_id: aurora_engine_types::types::u256_to_arr(&U256::from(chain_id)),
        owner_id: env.current_account_id(),
        bridge_prover_id: test_utils::str_to_account_id("bridge_prover.near"),
        upgrade_delay_blocks: 1,
    };

    engine::set_state(&mut io, new_args.into());

    let connector_args = InitCallArgs {
        prover_account: test_utils::str_to_account_id("prover.near"),
        eth_custodian_address: ETH_CUSTODIAN_ADDRESS.encode(),
        metadata: FungibleTokenMetadata::default(),
    };

    aurora_engine::connector::EthConnectorContract::create_contract(
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
        address: address.raw(),
        basic: evm::backend::Basic {
            balance: balance.raw(),
            nonce,
        },
        code,
        storage: std::iter::empty(),
        reset_storage: false,
    };

    let deposit_args = FinishDepositCallArgs {
        new_owner_id: aurora_account_id.clone(),
        amount: NEP141Wei::new(balance.raw().as_u128()),
        proof_key: String::new(),
        relayer_id: aurora_account_id.clone(),
        fee: 0.into(),
        msg: None,
    };

    // Delete the fake proof so that we can use it again.
    let proof_key = crate::prelude::storage::bytes_to_key(
        crate::prelude::storage::KeyPrefix::EthConnector,
        &[crate::prelude::storage::EthConnectorStorageId::UsedEvent as u8],
    );
    io.remove_storage(&proof_key);

    let mut connector = aurora_engine::connector::EthConnectorContract::init_instance(io).unwrap();
    connector
        .finish_deposit(
            aurora_account_id.clone(),
            aurora_account_id.clone(),
            deposit_args,
            NearGas::new(DEFAULT_GAS),
        )
        .map_err(unsafe_to_string)
        .unwrap();

    let transfer_args = NEP141FtOnTransferArgs {
        sender_id: aurora_account_id,
        amount: Balance::new(balance.raw().as_u128()),
        msg: format!(
            "aurora:{}{}",
            hex::encode(Wei::zero().to_bytes()),
            hex::encode(address.as_bytes())
        ),
    };
    connector.ft_on_transfer(&engine, &transfer_args).unwrap();

    engine.apply(std::iter::once(state_change), std::iter::empty(), false);
}

pub fn unsafe_to_string<E: AsRef<[u8]>>(e: E) -> String {
    String::from_utf8(e.as_ref().to_vec()).unwrap()
}

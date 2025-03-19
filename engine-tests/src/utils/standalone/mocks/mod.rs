use crate::utils;
use aurora_engine::engine;
use aurora_engine::engine::Engine;
#[cfg(not(feature = "ext-connector"))]
use aurora_engine::parameters::InitCallArgs;
use aurora_engine_sdk::env::{Env, DEFAULT_PREPAID_GAS};
use aurora_engine_sdk::io::IO;
#[cfg(not(feature = "ext-connector"))]
use aurora_engine_types::parameters::connector::FungibleTokenMetadata;
use aurora_engine_types::types::{Address, NearGas, Wei};
use aurora_engine_types::{account_id::AccountId, H256, U256};
use engine_standalone_storage::{BlockMetadata, Storage};

pub mod block;

#[cfg(not(feature = "ext-connector"))]
pub const ETH_CUSTODIAN_ADDRESS: Address =
    aurora_engine_types::types::make_address(0xd045f7e1, 0x9b2488924b97f9c145b5e51d0d895a65);
#[cfg(feature = "ext-connector")]
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

#[cfg(feature = "ext-connector")]
pub fn init_connector<I: IO + Copy>(io: I) {
    use aurora_engine::contract_methods::connector::external::{
        AdminControlled, EthConnectorContract,
    };
    use aurora_engine_types::parameters::connector::WithdrawSerializeType;

    let mut connector = EthConnectorContract::init(io).unwrap();
    connector.set_eth_connector_contract_account(&EXT_ETH_CONNECTOR.parse().unwrap());
    connector.set_withdraw_serialize_type(&WithdrawSerializeType::Borsh);
}

#[cfg(not(feature = "ext-connector"))]
pub fn init_connector<I: IO + Copy, E: Env>(io: I, env: &E) {
    let connector_args = InitCallArgs {
        prover_account: utils::str_to_account_id("prover.near"),
        eth_custodian_address: ETH_CUSTODIAN_ADDRESS.encode(),
        metadata: FungibleTokenMetadata::default(),
    };

    aurora_engine::contract_methods::connector::EthConnectorContract::create_contract(
        io,
        &env.current_account_id(),
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

    #[cfg(not(feature = "ext-connector"))]
    deposit(io, &env.current_account_id(), address, balance);

    engine.apply(std::iter::once(state_change), std::iter::empty(), false);
}

#[cfg(not(feature = "ext-connector"))]
fn deposit<I: IO + Copy>(mut io: I, aurora_account_id: &AccountId, address: Address, balance: Wei) {
    const DEFAULT_GAS: u64 = 300_000_000_000_000;
    let deposit_args = aurora_engine_types::parameters::connector::FinishDepositCallArgs {
        new_owner_id: aurora_account_id.clone(),
        amount: aurora_engine_types::types::NEP141Wei::new(balance.raw().as_u128()),
        proof_key: String::new(),
        relayer_id: aurora_account_id.clone(),
        fee: 0.into(),
        msg: None,
    };

    // Delete the fake proof so that we can use it again.
    let proof_key = crate::prelude::storage::bytes_to_key(
        crate::prelude::storage::KeyPrefix::EthConnector,
        &[crate::prelude::storage::EthConnectorStorageId::UsedEvent.into()],
    );
    io.remove_storage(&proof_key);

    let mut connector =
        aurora_engine::contract_methods::connector::EthConnectorContract::init(io).unwrap();
    connector
        .finish_deposit(
            aurora_account_id.clone(),
            aurora_account_id.clone(),
            deposit_args,
            aurora_engine_types::types::NearGas::new(DEFAULT_GAS),
        )
        .map_err(unsafe_to_string)
        .unwrap();

    let transfer_args = aurora_engine_types::parameters::connector::NEP141FtOnTransferArgs {
        sender_id: aurora_account_id.clone(),
        amount: aurora_engine_types::types::Balance::new(balance.raw().as_u128()),
        msg: format!(
            "aurora:{}{}",
            hex::encode(Wei::zero().to_bytes()),
            hex::encode(address.as_bytes())
        ),
    };
    connector.ft_on_transfer(&transfer_args).unwrap();
}

#[cfg(not(feature = "ext-connector"))]
fn unsafe_to_string<E: AsRef<[u8]>>(e: E) -> String {
    String::from_utf8(e.as_ref().to_vec()).unwrap()
}

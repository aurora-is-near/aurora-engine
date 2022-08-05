use aurora_engine::parameters::ViewCallArgs;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::types::NEP141Wei;
use borsh::{BorshDeserialize, BorshSerialize};
use libsecp256k1::{self, Message, PublicKey, SecretKey};
use near_primitives::runtime::config_store::RuntimeConfigStore;
use near_primitives::version::PROTOCOL_VERSION;
use near_primitives_core::config::VMConfig;
use near_primitives_core::contract::ContractCode;
use near_primitives_core::profile::ProfileData;
use near_primitives_core::runtime::fees::RuntimeFeesConfig;
use near_vm_logic::types::ReturnData;
use near_vm_logic::{VMContext, VMOutcome, ViewConfig};
use near_vm_runner::{MockCompiledContractCache, VMError};
use rlp::RlpStream;
use workspaces::{Worker, Contract};
use workspaces::network::{Sandbox, DevAccountDeployer};

use crate::prelude::fungible_token::{FungibleToken, FungibleTokenMetadata};
use crate::prelude::parameters::{InitCallArgs, NewCallArgs, SubmitResult, TransactionStatus};
use crate::prelude::transactions::{
    eip_1559::{self, SignedTransaction1559, Transaction1559},
    eip_2930::{self, SignedTransaction2930, Transaction2930},
    legacy::{LegacyEthSignedTransaction, TransactionLegacy},
};
use crate::prelude::{sdk, Address, Wei, H256, U256};
use crate::test_utils::solidity::{ContractConstructor, DeployedContract};

// TODO(Copied from #84): Make sure that there is only one Signer after both PR are merged.

pub fn origin() -> String {
    "aurora".to_string()
}

pub(crate) const SUBMIT: &str = "submit";
pub(crate) const CALL: &str = "call";
pub(crate) const DEPLOY_ERC20: &str = "deploy_erc20_token";
pub(crate) mod solidity;
pub(crate) mod erc20;
/* 
pub(crate) mod exit_precompile;
pub(crate) mod mocked_external;
pub(crate) mod one_inch;
pub(crate) mod random;
pub(crate) mod rust;
pub(crate) mod self_destruct;
pub(crate) mod solidity;
pub(crate) mod standalone;
pub(crate) mod standard_precompiles;
pub(crate) mod uniswap;
pub(crate) mod weth;
*/

pub struct Signer {
    pub nonce: u64,
    pub secret_key: SecretKey,
}

impl Signer {
    pub fn new(secret_key: SecretKey) -> Self {
        Self {
            nonce: 0,
            secret_key,
        }
    }

    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        let sk = SecretKey::random(&mut rng);
        Self::new(sk)
    }

    pub fn use_nonce(&mut self) -> u64 {
        let nonce = self.nonce;
        self.nonce += 1;
        nonce
    }
}




/// Wrapper around `ProfileData` to still include the wasm gas usage
/// (which was removed in https://github.com/near/nearcore/pull/4438).
#[derive(Default, Clone)]
pub(crate) struct ExecutionProfile {
    pub host_breakdown: ProfileData,
    wasm_gas: u64,
}

impl ExecutionProfile {
    pub fn new(outcome: &VMOutcome) -> Self {
        let wasm_gas =
            outcome.burnt_gas - outcome.profile.host_gas() - outcome.profile.action_gas();
        Self {
            host_breakdown: outcome.profile.clone(),
            wasm_gas,
        }
    }

    pub fn wasm_gas(&self) -> u64 {
        self.wasm_gas
    }

    pub fn all_gas(&self) -> u64 {
        self.wasm_gas + self.host_breakdown.host_gas() + self.host_breakdown.action_gas()
    }
}

const AURORA_WASM_FILEPATH: &str = "../mainnet-release.wasm";

pub const MAINNET_CHAIN_ID: u32 = 1313161556;

pub(crate) async fn deploy_evm() -> anyhow::Result<(Worker<Sandbox>, Contract)> {
    let worker = workspaces::sandbox().await?;
    let wasm = std::fs::read(AURORA_WASM_FILEPATH)?;
    let contract = worker.dev_deploy(&wasm).await?;

    // Record Chain metadata
    let args = NewCallArgs {
        chain_id: crate::prelude::u256_to_arr(&U256::from(MAINNET_CHAIN_ID)),
        owner_id: str_to_account_id("test.near"),
        bridge_prover_id: str_to_account_id("bridge_prover.near"),
        upgrade_delay_blocks: 1,
    };

    contract
        .call(&worker, "new")
        .args(args.try_to_vec().unwrap())
        .transact()
        .await?;

    // Setup new eth connector
    let init_evm = InitCallArgs {
        prover_account: str_to_account_id("prover.near"),
        eth_custodian_address: "d045f7e19B2488924B97F9c145b5E51D0D895A65".to_string(),
        metadata: FungibleTokenMetadata::default(),
    };

    contract
        .call(&worker, "new_eth_connector")
        .args(init_evm.try_to_vec().unwrap())
        .transact()
        .await?;

    return Ok((worker, contract));
}


pub(crate) fn transfer(to: Address, amount: Wei, nonce: U256) -> TransactionLegacy {
    TransactionLegacy {
        nonce,
        gas_price: Default::default(),
        gas_limit: u64::MAX.into(),
        to: Some(to),
        value: amount,
        data: Vec::new(),
    }
}

pub(crate) fn create_deploy_transaction(contract_bytes: Vec<u8>, nonce: U256) -> TransactionLegacy {
    let len = contract_bytes.len();
    if len > u16::MAX as usize {
        panic!("Cannot deploy a contract with that many bytes!");
    }
    let len = len as u16;
    // This bit of EVM byte code essentially says:
    // "If msg.value > 0 revert; otherwise return `len` amount of bytes that come after me
    // in the code." By prepending this to `contract_bytes` we create a valid EVM program which
    // returns `contract_bytes`, which is exactly what we want.
    let init_code = format!(
        "608060405234801561001057600080fd5b5061{}806100206000396000f300",
        hex::encode(len.to_be_bytes())
    );
    let data = hex::decode(init_code)
        .unwrap()
        .into_iter()
        .chain(contract_bytes.into_iter())
        .collect();

    TransactionLegacy {
        nonce,
        gas_price: Default::default(),
        gas_limit: u64::MAX.into(),
        to: None,
        value: Wei::zero(),
        data,
    }
}

pub(crate) fn create_eth_transaction(
    to: Option<Address>,
    value: crate::prelude::Wei,
    data: Vec<u8>,
    chain_id: Option<u64>,
    secret_key: &SecretKey,
) -> LegacyEthSignedTransaction {
    // nonce, gas_price and gas are not used by EVM contract currently
    let tx = TransactionLegacy {
        nonce: Default::default(),
        gas_price: Default::default(),
        gas_limit: u64::MAX.into(),
        to,
        value,
        data,
    };
    sign_transaction(tx, chain_id, secret_key)
}

pub(crate) fn as_view_call(tx: TransactionLegacy, sender: Address) -> ViewCallArgs {
    ViewCallArgs {
        sender,
        address: tx.to.unwrap(),
        amount: tx.value.to_bytes(),
        input: tx.data,
    }
}

pub(crate) fn sign_transaction(
    tx: TransactionLegacy,
    chain_id: Option<u64>,
    secret_key: &SecretKey,
) -> LegacyEthSignedTransaction {
    let mut rlp_stream = RlpStream::new();
    tx.rlp_append_unsigned(&mut rlp_stream, chain_id);
    let message_hash = sdk::keccak(rlp_stream.as_raw());
    let message = Message::parse_slice(message_hash.as_bytes()).unwrap();

    let (signature, recovery_id) = libsecp256k1::sign(&message, secret_key);
    let v: u64 = match chain_id {
        Some(chain_id) => (recovery_id.serialize() as u64) + 2 * chain_id + 35,
        None => (recovery_id.serialize() as u64) + 27,
    };
    let r = U256::from_big_endian(&signature.r.b32());
    let s = U256::from_big_endian(&signature.s.b32());
    LegacyEthSignedTransaction {
        transaction: tx,
        v,
        r,
        s,
    }
}

pub(crate) fn sign_access_list_transaction(
    tx: Transaction2930,
    secret_key: &SecretKey,
) -> SignedTransaction2930 {
    let mut rlp_stream = RlpStream::new();
    rlp_stream.append(&eip_2930::TYPE_BYTE);
    tx.rlp_append_unsigned(&mut rlp_stream);
    let message_hash = sdk::keccak(rlp_stream.as_raw());
    let message = Message::parse_slice(message_hash.as_bytes()).unwrap();

    let (signature, recovery_id) = libsecp256k1::sign(&message, secret_key);
    let r = U256::from_big_endian(&signature.r.b32());
    let s = U256::from_big_endian(&signature.s.b32());

    SignedTransaction2930 {
        transaction: tx,
        parity: recovery_id.serialize(),
        r,
        s,
    }
}

pub(crate) fn sign_eip_1559_transaction(
    tx: Transaction1559,
    secret_key: &SecretKey,
) -> SignedTransaction1559 {
    let mut rlp_stream = RlpStream::new();
    rlp_stream.append(&eip_1559::TYPE_BYTE);
    tx.rlp_append_unsigned(&mut rlp_stream);
    let message_hash = sdk::keccak(rlp_stream.as_raw());
    let message = Message::parse_slice(message_hash.as_bytes()).unwrap();

    let (signature, recovery_id) = libsecp256k1::sign(&message, secret_key);
    let r = U256::from_big_endian(&signature.r.b32());
    let s = U256::from_big_endian(&signature.s.b32());

    SignedTransaction1559 {
        transaction: tx,
        parity: recovery_id.serialize(),
        r,
        s,
    }
}

pub(crate) fn address_from_secret_key(sk: &SecretKey) -> Address {
    let pk = PublicKey::from_secret_key(sk);
    let hash = sdk::keccak(&pk.serialize()[1..]);
    Address::try_from_slice(&hash[12..]).unwrap()
}

pub(crate) fn parse_eth_gas(output: &VMOutcome) -> u64 {
    let submit_result_bytes = match &output.return_data {
        ReturnData::Value(bytes) => bytes.as_slice(),
        ReturnData::None | ReturnData::ReceiptIndex(_) => panic!("Unexpected ReturnData"),
    };
    let submit_result = SubmitResult::try_from_slice(submit_result_bytes).unwrap();
    submit_result.gas_used
}

pub(crate) fn validate_address_balance_and_nonce(
    runner: &AuroraRunner,
    address: Address,
    expected_balance: crate::prelude::Wei,
    expected_nonce: U256,
) {
    assert_eq!(runner.get_balance(address), expected_balance, "balance");
    assert_eq!(runner.get_nonce(address), expected_nonce, "nonce");
}

pub(crate) fn address_from_hex(address: &str) -> Address {
    let bytes = if address.starts_with("0x") {
        hex::decode(&address[2..]).unwrap()
    } else {
        hex::decode(address).unwrap()
    };

    Address::try_from_slice(&bytes).unwrap()
}

pub(crate) fn as_account_id(account_id: &str) -> near_primitives_core::types::AccountId {
    account_id.parse().unwrap()
}

pub(crate) fn str_to_account_id(account_id: &str) -> AccountId {
    use aurora_engine_types::str::FromStr;
    AccountId::from_str(account_id).unwrap()
}

pub fn unwrap_success(result: SubmitResult) -> Vec<u8> {
    match result.status {
        TransactionStatus::Succeed(ret) => ret,
        other => panic!("Unexpected status: {:?}", other),
    }
}

pub fn unwrap_success_slice(result: &SubmitResult) -> &[u8] {
    match &result.status {
        TransactionStatus::Succeed(ret) => &ret,
        other => panic!("Unexpected status: {:?}", other),
    }
}

pub fn unwrap_revert(result: SubmitResult) -> Vec<u8> {
    match result.status {
        TransactionStatus::Revert(ret) => ret,
        other => panic!("Unexpected status: {:?}", other),
    }
}

pub fn panic_on_fail(status: TransactionStatus) {
    match status {
        TransactionStatus::Succeed(_) => (),
        TransactionStatus::Revert(message) => panic!("{}", String::from_utf8_lossy(&message)),
        other => panic!("{}", String::from_utf8_lossy(other.as_ref())),
    }
}

pub fn assert_gas_bound(total_gas: u64, tgas_bound: u64) {
    // Add 1 to round up
    let tgas_used = (total_gas / 1_000_000_000_000) + 1;
    assert!(
        tgas_used == tgas_bound,
        "{} Tgas is not equal to {} Tgas",
        tgas_used,
        tgas_bound,
    );
}

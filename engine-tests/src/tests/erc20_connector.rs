use crate::prelude::{Address, Balance, Wei, WeiU256, U256};
use crate::test_utils;
use crate::test_utils::{create_eth_transaction, origin, AuroraRunner};
use aurora_engine::parameters::{CallArgs, FunctionCallArgsV2, SubmitResult};
use aurora_engine::proof::Proof;
use aurora_engine_transactions::legacy::LegacyEthSignedTransaction;
use borsh::{BorshDeserialize, BorshSerialize};
use ethabi::Token;
use libsecp256k1::SecretKey;
use near_vm_logic::VMOutcome;
use near_vm_runner::VMError;
use serde_json::json;
use sha3::Digest;

const INITIAL_BALANCE: Wei = Wei::new_u64(1000);
const INITIAL_NONCE: u64 = 0;
//const PROOF_DATA_NEAR: &'static str = r#"{"log_index":0,"log_entry_data":[248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,6,1,130,107,17,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"header_data":[249,2,10,160,177,33,112,26,26,176,12,12,163,2,249,133,245,12,51,201,55,50,148,156,122,67,27,26,101,178,36,153,54,100,53,137,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,124,28,230,160,8,239,64,193,62,78,177,68,166,204,116,240,224,174,172,126,160,197,65,5,202,188,134,5,164,246,19,133,35,57,28,114,241,186,81,123,163,166,161,24,32,157,168,170,13,108,58,61,46,160,6,199,163,13,91,119,225,39,168,255,213,10,107,252,143,246,138,241,108,139,59,35,187,185,162,223,53,108,222,73,181,109,160,27,154,49,63,26,170,15,177,97,255,6,204,84,221,234,197,159,172,114,47,148,126,32,199,241,127,101,120,182,51,52,100,185,1,0,0,0,8,0,0,0,0,0,0,0,32,0,0,0,0,0,2,0,8,0,0,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,8,32,0,32,0,0,128,0,2,0,0,0,1,0,32,0,0,0,2,0,0,0,0,32,0,0,0,0,0,4,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,128,64,0,0,0,0,1,32,0,0,0,0,0,0,96,32,0,64,0,0,0,128,1,0,0,0,0,1,0,0,0,8,0,0,0,18,32,0,0,64,145,1,8,0,4,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,33,16,0,128,0,0,0,0,0,0,128,0,2,0,0,0,0,0,0,0,0,0,0,2,0,80,0,0,0,0,0,0,0,0,1,128,0,8,0,0,0,0,4,0,0,0,128,2,0,32,0,128,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,16,0,8,0,0,0,0,0,0,0,0,0,0,128,0,64,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,132,25,1,227,23,131,157,85,14,131,122,18,0,131,75,91,132,132,96,174,58,224,140,115,112,105,100,101,114,49,48,1,2,8,230,160,188,212,199,183,154,22,223,85,103,215,24,122,240,235,79,129,44,93,184,88,161,218,79,5,44,226,106,100,50,40,163,97,136,155,158,202,3,149,91,200,78],"proof":[[248,113,160,46,156,31,85,241,226,241,13,5,56,73,146,176,67,195,109,6,189,172,104,44,103,44,88,32,15,181,152,136,29,121,252,160,191,48,87,174,71,151,208,114,164,150,51,200,171,90,90,106,46,200,79,77,222,145,95,89,141,137,138,149,67,73,8,87,128,128,128,128,128,128,160,175,9,219,77,174,13,247,133,55,172,92,185,202,7,160,10,204,112,44,133,36,96,30,234,235,134,30,209,205,166,212,255,128,128,128,128,128,128,128,128],[249,2,13,48,185,2,9,249,2,6,1,130,107,17,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0]]}"#;

pub struct CallResult {
    outcome: Option<VMOutcome>,
    error: Option<VMError>,
}

impl CallResult {
    fn check_ok(&self) {
        assert!(self.error.is_none());
    }

    fn value(&self) -> Vec<u8> {
        self.outcome
            .as_ref()
            .unwrap()
            .return_data
            .clone()
            .as_value()
            .unwrap()
    }

    fn submit_result(&self) -> SubmitResult {
        SubmitResult::try_from_slice(self.value().as_slice()).unwrap()
    }
}

fn keccak256(input: &[u8]) -> Vec<u8> {
    sha3::Keccak256::digest(input).to_vec()
}

fn get_selector(str_selector: &str) -> Vec<u8> {
    keccak256(str_selector.as_bytes())[..4].to_vec()
}

fn build_input(str_selector: &str, inputs: &[Token]) -> Vec<u8> {
    let sel = get_selector(str_selector);
    let inputs = ethabi::encode(inputs);
    [sel.as_slice(), inputs.as_slice()].concat().to_vec()
}

fn create_ethereum_address() -> Address {
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    test_utils::address_from_secret_key(&source_account)
}

pub struct EthereumAddress {
    pub secret_key: SecretKey,
    pub address: Address,
}

impl test_utils::AuroraRunner {
    pub fn new() -> Self {
        test_utils::deploy_evm()
    }

    pub fn make_call(
        &mut self,
        method_name: &str,
        caller_account_id: String,
        input: Vec<u8>,
    ) -> CallResult {
        let (outcome, error) = self.call(method_name, &caller_account_id, input);
        CallResult { outcome, error }
    }

    pub fn make_call_with_signer(
        &mut self,
        method_name: &str,
        caller_account_id: String,
        signer_account_id: String,
        input: Vec<u8>,
    ) -> CallResult {
        let (outcome, error) =
            self.call_with_signer(method_name, &caller_account_id, &signer_account_id, input);
        CallResult { outcome, error }
    }

    pub fn evm_call(&mut self, contract: Address, input: Vec<u8>, origin: String) -> CallResult {
        self.make_call(
            "call",
            origin,
            CallArgs::V2(FunctionCallArgsV2 {
                contract,
                value: WeiU256::default(),
                input,
            })
            .try_to_vec()
            .unwrap(),
        )
    }

    pub fn evm_submit(&mut self, input: LegacyEthSignedTransaction, origin: String) -> CallResult {
        self.make_call("submit", origin, rlp::encode(&input).to_vec())
    }

    pub fn deploy_erc20_token(&mut self, nep141: &str) -> Address {
        let result = self.make_call("deploy_erc20_token", origin(), nep141.try_to_vec().unwrap());

        result.check_ok();

        let raw_address: [u8; 20] = Vec::<u8>::try_from_slice(result.value().as_slice())
            .unwrap()
            .try_into()
            .unwrap();
        Address::try_from_slice(&raw_address).unwrap()
    }

    pub fn create_account(&mut self) -> EthereumAddress {
        let mut rng = rand::thread_rng();
        let source_account = SecretKey::random(&mut rng);
        let source_address = test_utils::address_from_secret_key(&source_account);
        self.create_address(source_address, INITIAL_BALANCE, INITIAL_NONCE.into());
        EthereumAddress {
            secret_key: source_account,
            address: source_address,
        }
    }

    pub fn balance_of(&mut self, token: Address, target: Address, origin: String) -> U256 {
        let input = build_input("balanceOf(address)", &[Token::Address(target.raw())]);
        let result = self.evm_call(token, input, origin);
        result.check_ok();
        let output = test_utils::unwrap_success(result.submit_result());
        U256::from_big_endian(output.as_slice())
    }

    pub fn mint(
        &mut self,
        token: Address,
        target: Address,
        amount: u64,
        origin: String,
    ) -> CallResult {
        let input = build_input(
            "mint(address,uint256)",
            &[
                Token::Address(target.raw()),
                Token::Uint(U256::from(amount)),
            ],
        );
        let result = self.evm_call(token, input, origin);
        result.check_ok();
        result
    }

    #[allow(dead_code)]
    pub fn admin(&mut self, token: Address, origin: String) -> CallResult {
        let input = build_input("admin()", &[]);
        let result = self.evm_call(token, input, origin);
        result.check_ok();
        result
    }

    pub fn transfer_erc20(
        &mut self,
        token: Address,
        sender: SecretKey,
        receiver: Address,
        amount: u64,
        origin: String,
    ) -> CallResult {
        // transfer(address recipient, uint256 amount)
        let input = build_input(
            "transfer(address,uint256)",
            &[
                Token::Address(receiver.raw()),
                Token::Uint(U256::from(amount)),
            ],
        );

        let input = create_eth_transaction(Some(token), Wei::zero(), input, None, &sender);

        let result = self.evm_submit(input, origin); // create_eth_transaction()
        result.check_ok();
        result
    }

    pub fn ft_on_transfer(
        &mut self,
        nep141: String,
        sender_id: String,
        relayer_id: String,
        amount: Balance,
        msg: String,
    ) -> String {
        let res = self.make_call_with_signer(
            "ft_on_transfer",
            nep141,
            relayer_id,
            json!({
                "sender_id": sender_id,
                "amount": amount.to_string(),
                "msg": msg
            })
            .to_string()
            .into_bytes(),
        );
        res.check_ok();
        String::from_utf8(res.value()).unwrap()
    }

    pub fn register_relayer(
        &mut self,
        relayer_account_id: String,
        relayer_address: Address,
    ) -> CallResult {
        self.make_call(
            "register_relayer",
            relayer_account_id,
            relayer_address.try_to_vec().unwrap(),
        )
    }
}

#[test]
fn test_deploy_erc20_token() {
    let mut runner = AuroraRunner::new();
    runner.deploy_erc20_token("tt.testnet");
}

#[test]
fn test_mint() {
    let mut runner = AuroraRunner::new();
    let token = runner.deploy_erc20_token("tt.testnet");
    let address = runner.create_account().address;
    let balance = runner.balance_of(token, address, origin());
    assert_eq!(balance, U256::from(0));
    let amount = 10;
    let _result = runner.mint(token, address, amount, origin());
    let balance = runner.balance_of(token, address, origin());
    assert_eq!(balance, U256::from(amount));
}

#[test]
fn test_mint_not_admin() {
    let mut runner = AuroraRunner::new();
    let token = runner.deploy_erc20_token("tt.testnet");
    let address = runner.create_account().address;
    let balance = runner.balance_of(token, address, origin());
    assert_eq!(balance, U256::from(0));
    let amount = 10;
    runner.mint(token, address, amount, "not_admin".to_string());
    let balance = runner.balance_of(token, address, origin());
    assert_eq!(balance, U256::from(0));
}

#[test]
fn test_ft_on_transfer() {
    let mut runner = AuroraRunner::new();
    // Standalone runner presently does not support ft_on_transfer
    runner.standalone_runner = None;
    let nep141 = "tt.testnet".to_string();
    let alice = "alice".to_string();
    let token = runner.deploy_erc20_token(&nep141);
    let amount = Balance::new(10);
    let recipient = runner.create_account().address;

    let balance = runner.balance_of(token, recipient, origin());
    assert_eq!(balance, U256::from(0));

    let res = runner.ft_on_transfer(nep141, alice.clone(), alice, amount, recipient.encode());
    // Transaction should succeed so return amount is 0
    assert_eq!(res, "\"0\"");

    let balance = runner.balance_of(token, recipient, origin());
    assert_eq!(balance, U256::from(amount.as_u128()));
}

#[test]
fn test_ft_on_transfer_fail() {
    let mut runner = AuroraRunner::new();
    let nep141 = "tt.testnet".to_string();
    let alice = "alice".to_string();
    let amount = Balance::new(10);

    let recipient = runner.create_account().address;

    let res = runner.ft_on_transfer(nep141, alice.clone(), alice, amount, recipient.encode());

    // Transaction should fail so it must return everything
    assert_eq!(res, format!("\"{}\"", amount));
}

#[ignore]
#[test]
fn test_relayer_charge_fee() {
    let mut runner = AuroraRunner::new();
    // Standalone runner presently does not support ft_on_transfer
    runner.standalone_runner = None;
    let amount = Balance::new(10);
    let fee = 51;
    let nep141 = "tt.testnet".to_string();
    let alice = "alice".to_string();
    let token = runner.deploy_erc20_token(&nep141);
    let recipient = runner.create_account().address;

    let recipient_balance = runner.get_balance(recipient);
    assert_eq!(recipient_balance, INITIAL_BALANCE);

    let relayer = create_ethereum_address();
    runner.register_relayer(alice.clone(), relayer);
    let relayer_balance = runner.get_balance(relayer);
    assert_eq!(relayer_balance, Wei::zero());

    let balance = runner.balance_of(token, recipient, origin());
    assert_eq!(balance, U256::from(0));

    let fee_encoded = &mut [0; 32];
    U256::from(fee).to_big_endian(fee_encoded);

    runner.ft_on_transfer(
        nep141,
        alice.clone(),
        alice,
        amount,
        recipient.encode() + &hex::encode(fee_encoded),
    );

    let recipient_balance_end = runner.get_balance(recipient);
    assert_eq!(
        recipient_balance_end,
        Wei::new_u64(INITIAL_BALANCE.raw().as_u64() - fee)
    );
    let relayer_balance = runner.get_balance(relayer);
    assert_eq!(relayer_balance, Wei::new_u64(fee));

    let balance = runner.balance_of(token, recipient, origin());
    assert_eq!(balance, U256::from(amount.as_u128()));
}

#[test]
fn test_transfer_erc20_token() {
    let mut runner = AuroraRunner::new();
    let token = runner.deploy_erc20_token("tt.testnet");
    let peer0 = runner.create_account();
    let peer1 = runner.create_account();

    let to_mint = 51;
    let to_transfer = 43;

    assert_eq!(
        runner.balance_of(token, peer0.address, origin()),
        U256::zero()
    );
    assert_eq!(
        runner.balance_of(token, peer1.address, origin()),
        U256::zero()
    );

    runner.mint(token, peer0.address, to_mint, origin());

    assert_eq!(
        runner.balance_of(token, peer0.address, origin()),
        U256::from(to_mint)
    );

    runner.transfer_erc20(
        token,
        peer0.secret_key,
        peer1.address,
        to_transfer,
        origin(),
    );
    assert_eq!(
        runner.balance_of(token, peer0.address, origin()),
        U256::from(to_mint - to_transfer)
    );

    assert_eq!(
        runner.balance_of(token, peer1.address, origin()),
        U256::from(to_transfer)
    );
}

// Simulation tests for exit to NEAR precompile.
// Note: `AuroraRunner` is not suitable for these tests because
// it does not execute promises; but `near-sdk-sim` does.
pub mod sim_tests {
    use crate::prelude::{Wei, WeiU256, U256};
    use crate::test_utils;
    use crate::test_utils::erc20::{ERC20Constructor, ERC20};
    use crate::test_utils::exit_precompile::TesterConstructor;

    use crate::tests::state_migration::{deploy_evm, AuroraAccount};
    use aurora_engine::fungible_token::FungibleTokenMetadata;
    use aurora_engine::parameters::{
        CallArgs, DeployErc20TokenArgs, FunctionCallArgsV2, SetEthConnectorContractAccountArgs,
        SubmitResult,
    };
    use aurora_engine::proof::Proof;
    use aurora_engine_types::account_id::AccountId;
    use aurora_engine_types::types::Address;
    use borsh::{BorshDeserialize, BorshSerialize};
    use near_sdk_sim::UserAccount;
    use serde_json::json;

    const FT_PATH: &str = "src/tests/res/fungible_token.wasm";
    const FT_TOTAL_SUPPLY: u128 = 1_000_000;
    const FT_TRANSFER_AMOUNT: u128 = 300_000;
    const FT_EXIT_AMOUNT: u128 = 100_000;
    const FT_ACCOUNT: &str = "test_token.root";
    const INITIAL_ETH_BALANCE: u64 = 777_777_777;
    const ETH_EXIT_AMOUNT: u64 = 111_111_111;
    const AURORA_ETH_CONNECTOR: &str = "aurora_eth_connector.root";

    #[test]
    fn test_ghsa_5c82_x4m4_hcj6_exploit() {
        let TestExitToNearEthContext {
            mut signer,
            signer_address,
            chain_id,
            tester_address: _,
            aurora,
        } = test_exit_to_near_eth_common();

        let constructor = test_utils::solidity::ContractConstructor::force_compile(
            "src/tests/res",
            "target/solidity_build",
            "exploit.sol",
            "Exploit",
        );
        let nonce = signer.use_nonce().into();
        let deploy_tx = constructor.deploy_without_constructor(nonce);
        let signed_tx = test_utils::sign_transaction(deploy_tx, Some(chain_id), &signer.secret_key);
        let deploy_result = aurora.call("submit", &rlp::encode(&signed_tx));
        let contract_address = match &deploy_result.status() {
            near_sdk_sim::transaction::ExecutionStatus::SuccessValue(bytes) => {
                let submit_result = SubmitResult::try_from_slice(bytes).unwrap();
                Address::try_from_slice(test_utils::unwrap_success_slice(&submit_result)).unwrap()
            }
            _ => panic!("Unknown result: {:?}", deploy_result),
        };
        let contract = constructor.deployed_at(contract_address);

        let nonce = signer.use_nonce().into();
        let hacker_account = "hacker.near";
        let hacker_account_bytes = hacker_account.as_bytes().to_vec();
        let mut exploit_tx = contract.call_method_with_args(
            "exploit",
            &[ethabi::Token::Bytes(hacker_account_bytes)],
            nonce,
        );
        exploit_tx.value = Wei::new_u64(ETH_EXIT_AMOUNT);
        let signed_tx =
            test_utils::sign_transaction(exploit_tx, Some(chain_id), &signer.secret_key);
        aurora
            .call("submit", &rlp::encode(&signed_tx))
            .assert_success();

        // check balances -- Hacker does not steal any funds!
        assert_eq!(
            nep_141_balance_of(
                aurora.contract.account_id.as_str(),
                &aurora.contract,
                &aurora,
            ),
            INITIAL_ETH_BALANCE as u128
        );
        assert_eq!(
            nep_141_balance_of(hacker_account, &aurora.contract, &aurora),
            0
        );
        assert_eq!(
            eth_balance_of(signer_address, &aurora),
            Wei::new_u64(INITIAL_ETH_BALANCE)
        );
    }

    #[test]
    fn test_exit_to_near() {
        // Deploy Aurora; deploy NEP-141; bridge NEP-141 to ERC-20 on Aurora
        let TestExitToNearContext {
            ft_owner,
            ft_owner_address,
            nep_141,
            erc20,
            aurora,
        } = test_exit_to_near_common();

        // Call exit function on ERC-20; observe ERC-20 burned + NEP-141 transferred
        exit_to_near(
            &ft_owner,
            ft_owner.account_id.as_str(),
            FT_EXIT_AMOUNT,
            &erc20,
            &aurora,
        );

        assert_eq!(
            nep_141_balance_of(ft_owner.account_id.as_str(), &nep_141, &aurora),
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT + FT_EXIT_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(aurora.contract.account_id.as_str(), &nep_141, &aurora),
            FT_TRANSFER_AMOUNT - FT_EXIT_AMOUNT
        );
        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora),
            (FT_TRANSFER_AMOUNT - FT_EXIT_AMOUNT).into()
        );
    }

    #[test]
    fn test_exit_to_near_refund() {
        // Deploy Aurora; deploy NEP-141; bridge NEP-141 to ERC-20 on Aurora
        let TestExitToNearContext {
            ft_owner,
            ft_owner_address,
            nep_141,
            erc20,
            aurora,
        } = test_exit_to_near_common();

        // Call exit on ERC-20; ft_transfer promise fails; expect refund on Aurora;
        exit_to_near(
            &ft_owner,
            // The ft_transfer will fail because this account is not registered with the NEP-141
            "unregistered.near",
            FT_EXIT_AMOUNT,
            &erc20,
            &aurora,
        );

        assert_eq!(
            nep_141_balance_of(ft_owner.account_id.as_str(), &nep_141, &aurora),
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(aurora.contract.account_id.as_str(), &nep_141, &aurora),
            FT_TRANSFER_AMOUNT
        );
        #[cfg(feature = "error_refund")]
        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora),
            FT_TRANSFER_AMOUNT.into()
        );
        // If the refund feature is not enabled then there is no refund in the EVM
        #[cfg(not(feature = "error_refund"))]
        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora),
            (FT_TRANSFER_AMOUNT - FT_EXIT_AMOUNT).into()
        );
    }

    #[test]
    fn test_exit_to_near_eth() {
        // Same test as above, but exit ETH instead of a bridged NEP-141

        let TestExitToNearEthContext {
            signer,
            signer_address,
            chain_id,
            tester_address,
            aurora,
        } = test_exit_to_near_eth_common();
        let exit_account_id = "any.near".to_owned();

        // call exit to near
        let input = super::build_input(
            "withdrawEthToNear(bytes)",
            &[ethabi::Token::Bytes(exit_account_id.as_bytes().to_vec())],
        );
        let exit_amount = ETH_EXIT_AMOUNT;
        let tx = test_utils::create_eth_transaction(
            Some(tester_address),
            Wei::new_u64(exit_amount),
            input,
            Some(chain_id),
            &signer.secret_key,
        );
        let res = aurora.call("submit", &rlp::encode(&tx));
        println!("{:#?}", res);
        println!("{:#?}", aurora.contract.account_id);
        res.assert_success();
        // check balances
        assert_eq!(
            nep_141_balance_of(
                aurora.contract.account_id.as_str(),
                &aurora.contract,
                &aurora,
            ),
            (INITIAL_ETH_BALANCE - exit_amount) as u128
        );

        assert_eq!(
            nep_141_balance_of(exit_account_id.as_str(), &aurora.contract, &aurora),
            exit_amount as u128
        );

        assert_eq!(
            eth_balance_of(signer_address, &aurora),
            Wei::new_u64(INITIAL_ETH_BALANCE - ETH_EXIT_AMOUNT)
        );
    }

    #[test]
    fn test_exit_to_near_eth_refund() {
        // Test the case where the ft_transfer promise from the exit call fails;
        // ensure ETH is refunded.

        let TestExitToNearEthContext {
            signer,
            signer_address,
            chain_id,
            tester_address,
            aurora,
        } = test_exit_to_near_eth_common();
        let exit_account_id = "any.near".to_owned();

        // Make the ft_transfer call fail by draining the Aurora account
        let transfer_args = json!({
            "receiver_id": "tmp.near",
            "amount": format!("{:?}", INITIAL_ETH_BALANCE),
            "memo": "null",
        });
        aurora
            .contract
            .call(
                aurora.contract.account_id(),
                "ft_transfer",
                transfer_args.to_string().as_bytes(),
                near_sdk_sim::DEFAULT_GAS,
                1,
            )
            .assert_success();

        // call exit to near
        let input = super::build_input(
            "withdrawEthToNear(bytes)",
            &[ethabi::Token::Bytes(exit_account_id.as_bytes().to_vec())],
        );
        let tx = test_utils::create_eth_transaction(
            Some(tester_address),
            Wei::new_u64(ETH_EXIT_AMOUNT),
            input,
            Some(chain_id),
            &signer.secret_key,
        );
        aurora.call("submit", &rlp::encode(&tx)).assert_success();

        // check balances
        assert_eq!(
            nep_141_balance_of(exit_account_id.as_str(), &aurora.contract, &aurora),
            0
        );

        #[cfg(feature = "error_refund")]
        assert_eq!(eth_balance_of(signer_address, &aurora), Wei::new_u64(100));
        // If the refund feature is not enabled then there is no refund in the EVM
        #[cfg(not(feature = "error_refund"))]
        assert_eq!(
            eth_balance_of(signer_address, &aurora),
            Wei::new_u64(INITIAL_ETH_BALANCE - ETH_EXIT_AMOUNT)
        );
    }

    fn test_exit_to_near_eth_common() -> TestExitToNearEthContext {
        let aurora = deploy_evm();
        let chain_id = test_utils::AuroraRunner::default().chain_id;
        let signer = test_utils::Signer::random();
        let signer_address = test_utils::address_from_secret_key(&signer.secret_key);
        aurora
            .call(
                "mint_account",
                &(signer_address, signer.nonce, INITIAL_ETH_BALANCE)
                    .try_to_vec()
                    .unwrap(),
            )
            .assert_success();
        let _nep141_contract = deploy_aurora_eth_connector(&aurora);

        assert_eq!(
            eth_balance_of(signer_address, &aurora),
            Wei::new_u64(INITIAL_ETH_BALANCE)
        );

        // deploy contract with simple exit to near method
        let constructor = TesterConstructor::load();
        let deploy_data = constructor.deploy(0, Address::zero()).data;
        let submit_result: SubmitResult = aurora.call("deploy_code", &deploy_data).unwrap_borsh();
        let tester_address =
            Address::try_from_slice(&test_utils::unwrap_success(submit_result)).unwrap();

        TestExitToNearEthContext {
            signer,
            signer_address,
            chain_id,
            tester_address,
            aurora,
        }
    }

    fn test_exit_to_near_common() -> TestExitToNearContext {
        // 1. deploy Aurora
        let aurora = deploy_evm();

        // 2. Create account
        let ft_owner = aurora.user.create_user(
            "ft_owner.root".parse().unwrap(),
            near_sdk_sim::STORAGE_AMOUNT,
        );
        let ft_owner_address =
            aurora_engine_sdk::types::near_account_to_evm_address(ft_owner.account_id.as_bytes());
        aurora
            .call(
                "mint_account",
                &(ft_owner_address, 0u64, INITIAL_ETH_BALANCE)
                    .try_to_vec()
                    .unwrap(),
            )
            .assert_success();

        // 3. Deploy NEP-141
        let nep_141 = deploy_nep_141(
            FT_ACCOUNT,
            ft_owner.account_id.as_ref(),
            FT_TOTAL_SUPPLY,
            &aurora,
        );

        assert_eq!(
            nep_141_balance_of(ft_owner.account_id.as_str(), &nep_141, &aurora),
            FT_TOTAL_SUPPLY
        );

        // 4. Deploy ERC-20 from NEP-141 and bridge value to Aurora
        let erc20 = deploy_erc20_from_nep_141(&nep_141, &aurora);
        transfer_nep_141_to_erc_20(
            &nep_141,
            &erc20,
            &ft_owner,
            ft_owner_address,
            FT_TRANSFER_AMOUNT,
            &aurora,
        );

        assert_eq!(
            nep_141_balance_of(ft_owner.account_id.as_str(), &nep_141, &aurora),
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(aurora.contract.account_id.as_str(), &nep_141, &aurora),
            FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora),
            FT_TRANSFER_AMOUNT.into()
        );

        TestExitToNearContext {
            ft_owner,
            ft_owner_address,
            nep_141,
            erc20,
            aurora,
        }
    }

    fn exit_to_near(
        source: &near_sdk_sim::UserAccount,
        dest: &str,
        amount: u128,
        erc20: &ERC20,
        aurora: &AuroraAccount,
    ) {
        let input = super::build_input(
            "withdrawToNear(bytes,uint256)",
            &[
                ethabi::Token::Bytes(dest.as_bytes().to_vec()),
                ethabi::Token::Uint(amount.into()),
            ],
        );
        let call_args = CallArgs::V2(FunctionCallArgsV2 {
            contract: erc20.0.address,
            value: WeiU256::default(),
            input,
        });
        source
            .call(
                aurora.contract.account_id(),
                "call",
                &call_args.try_to_vec().unwrap(),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .assert_success();
    }

    pub(crate) fn transfer_nep_141_to_erc_20(
        nep_141: &near_sdk_sim::UserAccount,
        erc20: &ERC20,
        source: &near_sdk_sim::UserAccount,
        dest: Address,
        amount: u128,
        aurora: &AuroraAccount,
    ) {
        let transfer_args = json!({
            "receiver_id": aurora.contract.account_id.as_str(),
            "amount": format!("{:?}", amount),
            "memo": "null",
        });
        source
            .call(
                nep_141.account_id(),
                "ft_transfer",
                transfer_args.to_string().as_bytes(),
                near_sdk_sim::DEFAULT_GAS,
                1,
            )
            .assert_success();

        let mint_tx = erc20.mint(dest, amount.into(), 0.into());
        let call_args = CallArgs::V2(FunctionCallArgsV2 {
            contract: erc20.0.address,
            value: WeiU256::default(),
            input: mint_tx.data,
        });
        aurora
            .contract
            .call(
                aurora.contract.account_id(),
                "call",
                &call_args.try_to_vec().unwrap(),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .assert_success();
    }

    fn eth_balance_of(address: Address, aurora: &AuroraAccount) -> Wei {
        let result = aurora.call("get_balance", address.as_bytes());

        result.assert_success();
        match result.status() {
            near_sdk_sim::transaction::ExecutionStatus::SuccessValue(bytes) => {
                Wei::new(U256::from_big_endian(&bytes))
            }
            _ => unreachable!(),
        }
    }

    fn erc20_balance(erc20: &ERC20, address: Address, aurora: &AuroraAccount) -> U256 {
        let balance_tx = erc20.balance_of(address, 0.into());
        let call_args = CallArgs::V2(FunctionCallArgsV2 {
            contract: erc20.0.address,
            value: WeiU256::default(),
            input: balance_tx.data,
        });
        let result = aurora.call("call", &call_args.try_to_vec().unwrap());
        let submit_result: SubmitResult = result.unwrap_borsh();
        U256::from_big_endian(&test_utils::unwrap_success(submit_result))
    }

    pub(crate) fn deploy_erc20_from_nep_141(
        nep_141: &near_sdk_sim::UserAccount,
        aurora: &AuroraAccount,
    ) -> ERC20 {
        let args = DeployErc20TokenArgs {
            nep141: nep_141.account_id().as_str().parse().unwrap(),
        };
        let result = aurora.call("deploy_erc20_token", &args.try_to_vec().unwrap());
        let addr_bytes: Vec<u8> = result.unwrap_borsh();
        let address = Address::try_from_slice(&addr_bytes).unwrap();
        let abi = ERC20Constructor::load().0.abi;
        ERC20(crate::test_utils::solidity::DeployedContract { abi, address })
    }

    pub fn nep_141_balance_of(
        account_id: &str,
        nep_141: &near_sdk_sim::UserAccount,
        aurora: &AuroraAccount,
    ) -> u128 {
        aurora
            .user
            .call(
                nep_141.account_id(),
                "ft_balance_of",
                json!({ "account_id": account_id }).to_string().as_bytes(),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .unwrap_json_value()
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    }

    /// Deploys the standard FT implementation:
    /// https://github.com/near/near-sdk-rs/blob/master/examples/fungible-token/ft/src/lib.rs
    pub fn deploy_nep_141(
        nep_141_account_id: &str,
        token_owner: &str,
        amount: u128,
        aurora: &AuroraAccount,
    ) -> UserAccount {
        let contract_bytes = std::fs::read(FT_PATH).unwrap();

        let contract_account = aurora.user.deploy(
            &contract_bytes,
            nep_141_account_id.parse().unwrap(),
            5 * near_sdk_sim::STORAGE_AMOUNT,
        );

        let init_args = json!({
            "owner_id": token_owner,
            "total_supply": format!("{:?}", amount),
        })
        .to_string();

        aurora
            .user
            .call(
                contract_account.account_id(),
                "new_default_meta",
                init_args.as_bytes(),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .assert_success();

        // Need to register Aurora contract so that it can receive tokens
        let args = json!({
            "account_id": &aurora.contract.account_id,
        })
        .to_string();
        aurora
            .user
            .call(
                contract_account.account_id(),
                "storage_deposit",
                args.as_bytes(),
                near_sdk_sim::DEFAULT_GAS,
                near_sdk_sim::STORAGE_AMOUNT,
            )
            .assert_success();

        contract_account
    }

    pub fn get_aurora_eth_connector_contract() -> Vec<u8> {
        use std::path::Path;
        let contract_path = Path::new("../engine-tests-connector/etc/aurora-eth-connector");
        std::fs::read(contract_path.join("bin/aurora-eth-connector-test.wasm")).unwrap()
    }

    pub fn deploy_aurora_eth_connector(aurora: &AuroraAccount) -> UserAccount {
        #[derive(BorshSerialize, BorshDeserialize)]
        pub struct InitCallArgs {
            pub prover_account: AccountId,
            pub eth_custodian_address: String,
            pub metadata: FungibleTokenMetadata,
            pub account_with_access_right: AccountId,
        }

        let contract_bytes = get_aurora_eth_connector_contract();

        let contract_account = aurora.user.deploy(
            &contract_bytes,
            AURORA_ETH_CONNECTOR.parse().unwrap(),
            15 * near_sdk_sim::STORAGE_AMOUNT,
        );

        let metadata = FungibleTokenMetadata::default();
        let metadata = json!({
            "spec": metadata.spec,
            "name": metadata.name,
            "symbol": metadata.symbol,
            "icon": metadata.icon,
            "reference": metadata.reference,
            "decimals": metadata.decimals,
        });
        let init_args = json!( {
            "prover_account": AccountId::new(AURORA_ETH_CONNECTOR).unwrap(),
            "eth_custodian_address": "096DE9C2B8A5B8c22cEe3289B101f6960d68E51E".to_string(),
            "metadata": metadata,
            "account_with_access_right": AccountId::new(aurora.contract.account_id.as_ref())
                .unwrap(),
        });

        contract_account
            .call(
                contract_account.account_id(),
                "new",
                init_args.to_string().as_bytes(),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .assert_success();

        let args = SetEthConnectorContractAccountArgs {
            account: AccountId::new(contract_account.account_id.as_ref()).unwrap(),
        };
        aurora
            .contract
            .call(
                aurora.contract.account_id.clone(),
                "set_eth_connector_contract_account",
                &args.try_to_vec().unwrap(),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .assert_success();

        let proof: Proof = super::create_test_proof(
            INITIAL_ETH_BALANCE,
            aurora.contract.account_id().as_ref(),
            "096de9c2b8a5b8c22cee3289b101f6960d68e51e",
        );
        aurora
            .contract
            .call(
                aurora.contract.account_id(),
                "deposit",
                &proof.try_to_vec().unwrap(),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .assert_success();

        let balance: u64 = aurora
            .contract
            .call(
                aurora.contract.account_id(),
                "ft_balance_of",
                json!({ "account_id": aurora.contract.account_id() })
                    .to_string()
                    .as_bytes(),
                near_sdk_sim::DEFAULT_GAS,
                0,
            )
            .unwrap_json_value()
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(balance, INITIAL_ETH_BALANCE);

        contract_account
    }

    struct TestExitToNearContext {
        ft_owner: near_sdk_sim::UserAccount,
        ft_owner_address: Address,
        nep_141: near_sdk_sim::UserAccount,
        erc20: ERC20,
        aurora: AuroraAccount,
    }

    struct TestExitToNearEthContext {
        signer: test_utils::Signer,
        signer_address: Address,
        chain_id: u64,
        tester_address: Address,
        aurora: AuroraAccount,
    }
}

fn create_test_proof(deposit_amount: u64, recipient_id: &str, custodian_address: &str) -> Proof {
    use aurora_engine::deposit_event::TokenMessageData;
    use aurora_engine_types::types::{Fee, NEP141Wei};

    let eth_custodian_address: Address = Address::decode(custodian_address).unwrap();

    let message = recipient_id.to_string();
    let fee: Fee = Fee::new(NEP141Wei::new(0));
    let token_message_data =
        TokenMessageData::parse_event_message_and_prepare_token_message_data(&message, fee)
            .unwrap();

    let deposit_event = aurora_engine::deposit_event::DepositedEvent {
        eth_custodian_address,
        sender: Address::zero(),
        token_message_data,
        amount: NEP141Wei::new(deposit_amount as u128),
        fee,
    };

    let event_schema = ethabi::Event {
        name: aurora_engine::deposit_event::DEPOSITED_EVENT.into(),
        inputs: aurora_engine::deposit_event::DepositedEvent::event_params(),
        anonymous: false,
    };
    let log_entry = aurora_engine::log_entry::LogEntry {
        address: eth_custodian_address.raw(),
        topics: vec![
            event_schema.signature(),
            // the sender is not important
            crate::prelude::H256::zero(),
        ],
        data: ethabi::encode(&[
            ethabi::Token::String(message),
            ethabi::Token::Uint(U256::from(deposit_event.amount.as_u128())),
            ethabi::Token::Uint(U256::from(deposit_event.fee.as_u128())),
        ]),
    };
    Proof {
        log_index: 1,
        log_entry_data: rlp::encode(&log_entry).to_vec(),
        receipt_index: 1,
        receipt_data: Vec::new(),
        header_data: Vec::new(),
        proof: Vec::new(),
    }
}

use crate::parameters::{FunctionCallArgs, SubmitResult};
use crate::prelude::*;
use crate::test_utils;
use crate::test_utils::{create_eth_transaction, origin, AuroraRunner};
use crate::transaction::LegacyEthSignedTransaction;
use crate::types::{AccountId, Balance, RawAddress, Wei};
use borsh::{BorshDeserialize, BorshSerialize};
use ethabi::Token;
use near_vm_logic::VMOutcome;
use near_vm_runner::VMError;
use secp256k1::SecretKey;
use serde_json::json;
use sha3::Digest;

const INITIAL_BALANCE: Wei = Wei::new_u64(1000);
const INITIAL_NONCE: u64 = 0;

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
    pub address: RawAddress,
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
        let (outcome, error) = self.call(method_name, caller_account_id, input);
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
            self.call_with_signer(method_name, caller_account_id, signer_account_id, input);
        CallResult { outcome, error }
    }

    pub fn evm_call(
        &mut self,
        contract: RawAddress,
        input: Vec<u8>,
        origin: AccountId,
    ) -> CallResult {
        self.make_call(
            "call",
            origin,
            (FunctionCallArgs { contract, input }).try_to_vec().unwrap(),
        )
    }

    pub fn evm_submit(
        &mut self,
        input: LegacyEthSignedTransaction,
        origin: AccountId,
    ) -> CallResult {
        self.make_call("submit", origin, rlp::encode(&input).to_vec())
    }

    pub fn deploy_erc20_token(&mut self, nep141: &AccountId) -> RawAddress {
        let result = self.make_call("deploy_erc20_token", origin(), nep141.try_to_vec().unwrap());

        result.check_ok();

        Vec::<u8>::try_from_slice(result.value().as_slice())
            .unwrap()
            .try_into()
            .unwrap()
    }

    pub fn create_account(&mut self) -> EthereumAddress {
        let mut rng = rand::thread_rng();
        let source_account = SecretKey::random(&mut rng);
        let source_address = test_utils::address_from_secret_key(&source_account);
        self.create_address(source_address, INITIAL_BALANCE.into(), INITIAL_NONCE.into());
        EthereumAddress {
            secret_key: source_account,
            address: source_address.into(),
        }
    }

    pub fn balance_of(&mut self, token: RawAddress, target: RawAddress, origin: AccountId) -> U256 {
        let input = build_input("balanceOf(address)", &[Token::Address(target.into())]);
        let result = self.evm_call(token, input, origin);
        result.check_ok();
        U256::from_big_endian(result.submit_result().result.as_slice())
    }

    pub fn mint(
        &mut self,
        token: RawAddress,
        target: RawAddress,
        amount: u64,
        origin: AccountId,
    ) -> CallResult {
        let input = build_input(
            "mint(address,uint256)",
            &[
                Token::Address(target.into()),
                Token::Uint(U256::from(amount).into()),
            ],
        );
        let result = self.evm_call(token, input, origin);
        result.check_ok();
        result
    }

    #[allow(dead_code)]
    pub fn admin(&mut self, token: RawAddress, origin: AccountId) -> CallResult {
        let input = build_input("admin()", &[]);
        let result = self.evm_call(token, input, origin);
        result.check_ok();
        result
    }

    pub fn transfer_erc20(
        &mut self,
        token: RawAddress,
        sender: SecretKey,
        receiver: RawAddress,
        amount: u64,
        origin: AccountId,
    ) -> CallResult {
        // transfer(address recipient, uint256 amount)
        let input = build_input(
            "transfer(address,uint256)",
            &[
                Token::Address(receiver.into()),
                Token::Uint(U256::from(amount)),
            ],
        );

        let input = create_eth_transaction(Some(token.into()), Wei::zero(), input, None, &sender);

        let result = self.evm_submit(input, origin); // create_eth_transaction()
        result.check_ok();
        result
    }

    pub fn ft_on_transfer(
        &mut self,
        nep141: AccountId,
        sender_id: AccountId,
        relayer_id: AccountId,
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
        relayer_account_id: AccountId,
        relayer_address: Address,
    ) -> CallResult {
        self.make_call(
            "register_relayer",
            relayer_account_id,
            relayer_address.as_fixed_bytes().try_to_vec().unwrap(),
        )
    }
}

#[test]
fn test_deploy_erc20_token() {
    let mut runner = AuroraRunner::new();
    runner.deploy_erc20_token(&"tt.testnet".to_string());
}

#[test]
fn test_mint() {
    let mut runner = AuroraRunner::new();
    let token = runner.deploy_erc20_token(&"tt.testnet".to_string());
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
    let token = runner.deploy_erc20_token(&"tt.testnet".to_string());
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
    let nep141 = "tt.testnet".to_string();
    let alice = "alice".to_string();
    let token = runner.deploy_erc20_token(&nep141);
    let amount = 10;
    let recipient = runner.create_account().address;

    let balance = runner.balance_of(token, recipient, origin());
    assert_eq!(balance, U256::from(0));

    let res = runner.ft_on_transfer(nep141, alice.clone(), alice, amount, hex::encode(recipient));
    // Transaction should succeed so return amount is 0
    assert_eq!(res, "\"0\"");

    let balance = runner.balance_of(token, recipient, origin());
    assert_eq!(balance, U256::from(amount));
}

#[test]
fn test_ft_on_transfer_fail() {
    let mut runner = AuroraRunner::new();
    let nep141 = "tt.testnet".to_string();
    let alice = "alice".to_string();
    let amount = 10;

    let recipient = runner.create_account().address;

    let res = runner.ft_on_transfer(nep141, alice.clone(), alice, amount, hex::encode(recipient));

    // Transaction should fail so it must return everything
    assert_eq!(res, format!("\"{}\"", amount.to_string()));
}

#[test]
fn test_relayer_charge_fee() {
    let mut runner = AuroraRunner::new();
    let amount = 10;
    let fee = 51;
    let nep141 = "tt.testnet".to_string();
    let alice = "alice".to_string();
    let token = runner.deploy_erc20_token(&nep141);
    let recipient = runner.create_account().address;

    let recipient_balance = runner.get_balance(recipient.into());
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
        hex::encode(recipient) + &hex::encode(fee_encoded),
    );

    let recipient_balance_end = runner.get_balance(recipient.into());
    assert_eq!(
        recipient_balance_end,
        Wei::new_u64(INITIAL_BALANCE.raw().as_u64() - fee)
    );
    let relayer_balance = runner.get_balance(relayer);
    assert_eq!(relayer_balance, Wei::new_u64(fee));

    let balance = runner.balance_of(token, recipient, origin());
    assert_eq!(balance, U256::from(amount));
}

#[test]
fn test_transfer_erc20_token() {
    let mut runner = AuroraRunner::new();
    let token = runner.deploy_erc20_token(&"tt.testnet".to_string());
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

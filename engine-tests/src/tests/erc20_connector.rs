use crate::prelude::{Address, Balance, Wei, WeiU256, U256};
use crate::test_utils::{self, create_eth_transaction, AuroraRunner, ORIGIN};
use aurora_engine::engine::EngineError;
use aurora_engine::parameters::{CallArgs, FunctionCallArgsV2};
use aurora_engine_transactions::legacy::LegacyEthSignedTransaction;
use aurora_engine_types::parameters::engine::{SubmitResult, TransactionStatus};
use borsh::{BorshDeserialize, BorshSerialize};
use ethabi::Token;
use libsecp256k1::SecretKey;
use near_vm_logic::VMOutcome;
use serde_json::json;
use sha3::Digest;

const INITIAL_BALANCE: Wei = Wei::new_u64(1000);
const INITIAL_NONCE: u64 = 0;

fn keccak256(input: &[u8]) -> Vec<u8> {
    sha3::Keccak256::digest(input).to_vec()
}

fn get_selector(str_selector: &str) -> Vec<u8> {
    keccak256(str_selector.as_bytes())[..4].to_vec()
}

fn build_input(str_selector: &str, inputs: &[Token]) -> Vec<u8> {
    let sel = get_selector(str_selector);
    let inputs = ethabi::encode(inputs);
    [sel.as_slice(), inputs.as_slice()].concat()
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

impl AuroraRunner {
    pub fn new() -> Self {
        test_utils::deploy_evm()
    }

    pub fn make_call(
        &mut self,
        method_name: &str,
        caller_account_id: &str,
        input: Vec<u8>,
    ) -> Result<VMOutcome, EngineError> {
        self.call(method_name, caller_account_id, input)
    }

    pub fn make_call_with_signer(
        &mut self,
        method_name: &str,
        caller_account_id: &str,
        signer_account_id: &str,
        input: Vec<u8>,
    ) -> Result<VMOutcome, EngineError> {
        self.call_with_signer(method_name, caller_account_id, signer_account_id, input)
    }

    pub fn evm_call(
        &mut self,
        contract: Address,
        input: Vec<u8>,
        origin: &str,
    ) -> Result<VMOutcome, EngineError> {
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

    pub fn evm_submit(
        &mut self,
        input: &LegacyEthSignedTransaction,
        origin: &str,
    ) -> Result<VMOutcome, EngineError> {
        self.make_call("submit", origin, rlp::encode(input).to_vec())
    }

    pub fn deploy_erc20_token(&mut self, nep141: &str) -> Address {
        let result = self
            .make_call("deploy_erc20_token", ORIGIN, nep141.try_to_vec().unwrap())
            .unwrap();

        Vec::try_from_slice(&result.return_data.as_value().unwrap())
            .unwrap()
            .try_into()
            .map(Address::from_array)
            .unwrap()
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

    pub fn balance_of(&mut self, token: Address, target: Address, origin: &str) -> U256 {
        let input = build_input("balanceOf(address)", &[Token::Address(target.raw())]);
        let result = self.evm_call(token, input, origin).unwrap();
        let output = result.return_data.as_value().unwrap();
        let result = SubmitResult::try_from_slice(&output).unwrap();

        match result.status {
            TransactionStatus::Succeed(bytes) => U256::from_big_endian(&bytes),
            other => panic!("Wrong EVM transaction status: {other:?}"),
        }
    }

    pub fn mint(
        &mut self,
        token: Address,
        target: Address,
        amount: u64,
        origin: &str,
    ) -> Result<VMOutcome, EngineError> {
        let input = build_input(
            "mint(address,uint256)",
            &[
                Token::Address(target.raw()),
                Token::Uint(U256::from(amount)),
            ],
        );

        self.evm_call(token, input, origin)
    }

    #[allow(dead_code)]
    pub fn admin(&mut self, token: Address, origin: &str) -> Result<VMOutcome, EngineError> {
        let input = build_input("admin()", &[]);
        self.evm_call(token, input, origin)
    }

    pub fn transfer_erc20(
        &mut self,
        token: Address,
        sender: SecretKey,
        receiver: Address,
        amount: u64,
        origin: &str,
    ) -> Result<VMOutcome, EngineError> {
        // transfer(address recipient, uint256 amount)
        let input = build_input(
            "transfer(address,uint256)",
            &[
                Token::Address(receiver.raw()),
                Token::Uint(U256::from(amount)),
            ],
        );
        let input = create_eth_transaction(Some(token), Wei::zero(), input, None, &sender);
        self.evm_submit(&input, origin) // create_eth_transaction()
    }

    pub fn ft_on_transfer(
        &mut self,
        nep141: &str,
        sender_id: &str,
        relayer_id: &str,
        amount: Balance,
        msg: &str,
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
        assert!(res.is_ok());
        String::from_utf8(res.unwrap().return_data.as_value().unwrap()).unwrap()
    }

    pub fn register_relayer(
        &mut self,
        relayer_account_id: &str,
        relayer_address: Address,
    ) -> Result<VMOutcome, EngineError> {
        self.make_call(
            "register_relayer",
            relayer_account_id,
            relayer_address.try_to_vec().unwrap(),
        )
    }

    pub fn factory_set_wnear_address(
        &mut self,
        wnear_address: Address,
    ) -> Result<VMOutcome, EngineError> {
        self.make_call(
            "factory_set_wnear_address",
            ORIGIN,
            wnear_address.try_to_vec().unwrap(),
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
    let balance = runner.balance_of(token, address, ORIGIN);
    assert_eq!(balance, U256::from(0));
    let amount = 10;
    let _result = runner.mint(token, address, amount, ORIGIN);
    let balance = runner.balance_of(token, address, ORIGIN);
    assert_eq!(balance, U256::from(amount));
}

#[test]
fn test_mint_not_admin() {
    let mut runner = AuroraRunner::new();
    let token = runner.deploy_erc20_token("tt.testnet");
    let address = runner.create_account().address;
    let balance = runner.balance_of(token, address, ORIGIN);
    assert_eq!(balance, U256::from(0));
    let amount = 10;
    runner.mint(token, address, amount, "not_admin").unwrap();
    let balance = runner.balance_of(token, address, ORIGIN);
    assert_eq!(balance, U256::from(0));
}

#[test]
fn test_ft_on_transfer() {
    let mut runner = AuroraRunner::new();
    // Standalone runner presently does not support ft_on_transfer
    runner.standalone_runner = None;
    let nep141 = "tt.testnet";
    let alice = "alice";
    let token = runner.deploy_erc20_token(nep141);
    let amount = Balance::new(10);
    let recipient = runner.create_account().address;

    let balance = runner.balance_of(token, recipient, ORIGIN);
    assert_eq!(balance, U256::from(0));

    let res = runner.ft_on_transfer(nep141, alice, alice, amount, &recipient.encode());
    // Transaction should succeed so return amount is 0
    assert_eq!(res, "\"0\"");

    let balance = runner.balance_of(token, recipient, ORIGIN);
    assert_eq!(balance, U256::from(amount.as_u128()));
}

#[test]
fn test_ft_on_transfer_fail() {
    let mut runner = AuroraRunner::new();
    let nep141 = "tt.testnet";
    let alice = "alice";
    let amount = Balance::new(10);
    let recipient = runner.create_account().address;
    let res = runner.ft_on_transfer(nep141, alice, alice, amount, &recipient.encode());

    // Transaction should fail so it must return everything
    assert_eq!(res, format!("\"{amount}\""));
}

#[ignore]
#[test]
fn test_relayer_charge_fee() {
    let mut runner = AuroraRunner::new();
    // Standalone runner presently does not support ft_on_transfer
    runner.standalone_runner = None;
    let amount = Balance::new(10);
    let fee = 51;
    let nep141 = "tt.testnet";
    let alice = "alice";
    let token = runner.deploy_erc20_token(nep141);
    let recipient = runner.create_account().address;

    let recipient_balance = runner.get_balance(recipient);
    assert_eq!(recipient_balance, INITIAL_BALANCE);

    let relayer = create_ethereum_address();
    runner.register_relayer(alice, relayer).unwrap();
    let relayer_balance = runner.get_balance(relayer);
    assert_eq!(relayer_balance, Wei::zero());

    let balance = runner.balance_of(token, recipient, ORIGIN);
    assert_eq!(balance, U256::from(0));

    let fee_encoded = &mut [0; 32];
    U256::from(fee).to_big_endian(fee_encoded);

    runner.ft_on_transfer(
        nep141,
        alice,
        alice,
        amount,
        &format!("{}{}", recipient.encode(), hex::encode(fee_encoded)),
    );

    let recipient_balance_end = runner.get_balance(recipient);
    assert_eq!(
        recipient_balance_end,
        Wei::new_u64(INITIAL_BALANCE.raw().as_u64() - fee)
    );
    let relayer_balance = runner.get_balance(relayer);
    assert_eq!(relayer_balance, Wei::new_u64(fee));

    let balance = runner.balance_of(token, recipient, ORIGIN);
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
        runner.balance_of(token, peer0.address, ORIGIN),
        U256::zero()
    );
    assert_eq!(
        runner.balance_of(token, peer1.address, ORIGIN),
        U256::zero()
    );

    runner.mint(token, peer0.address, to_mint, ORIGIN).unwrap();

    assert_eq!(
        runner.balance_of(token, peer0.address, ORIGIN),
        U256::from(to_mint)
    );

    runner
        .transfer_erc20(token, peer0.secret_key, peer1.address, to_transfer, ORIGIN)
        .unwrap();
    assert_eq!(
        runner.balance_of(token, peer0.address, ORIGIN),
        U256::from(to_mint - to_transfer)
    );

    assert_eq!(
        runner.balance_of(token, peer1.address, ORIGIN),
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
    use aurora_engine::parameters::{
        CallArgs, DeployErc20TokenArgs, FunctionCallArgsV2, SubmitResult,
    };
    use aurora_engine_types::types::Address;
    use borsh::{BorshDeserialize, BorshSerialize};
    use near_sdk_sim::{ExecutionResult, UserAccount};
    use serde_json::json;

    const FT_PATH: &str = "src/tests/res/fungible_token.wasm";
    const FT_TOTAL_SUPPLY: u128 = 1_000_000;
    const FT_TRANSFER_AMOUNT: u128 = 300_000;
    const FT_EXIT_AMOUNT: u128 = 100_000;
    const FT_ACCOUNT: &str = "test_token.root";
    const INITIAL_ETH_BALANCE: u64 = 777_777_777;
    const ETH_EXIT_AMOUNT: u64 = 111_111_111;

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
            _ => panic!("Unknown result: {deploy_result:?}"),
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
            u128::from(INITIAL_ETH_BALANCE)
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
            ..
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
    fn test_exit_to_near_wnear_unwrapped() {
        // Deploy Aurora; deploy wnear; bridge wnear to ERC-20 on Aurora
        let TestExitToNearContext {
            ft_owner,
            ft_owner_address,
            aurora,
            wnear,
            wnear_erc20,
            ..
        } = test_exit_to_near_common();

        let ft_owner_balance = get_account_balance(ft_owner.account_id.as_str(), &aurora);

        // Call exit function on ERC-20; observe ERC-20 burned + near tokens transferred
        let result = exit_to_near(
            &ft_owner,
            &format!("{}:unwrap", ft_owner.account_id.as_str()),
            FT_EXIT_AMOUNT,
            &wnear_erc20,
            &aurora,
        );
        let total_tokens_burnt: u128 = result
            .promise_results()
            .iter()
            .map(|r| r.as_ref().unwrap().outcome().tokens_burnt)
            .sum();

        // Check that the wnear tokens are properly unwrapped and transferred to `ft_owner`
        assert_eq!(
            get_account_balance(ft_owner.account_id.as_str(), &aurora),
            ft_owner_balance - total_tokens_burnt + FT_EXIT_AMOUNT
        );

        // Check wnear balances
        assert_eq!(
            nep_141_balance_of(ft_owner.account_id.as_str(), &wnear, &aurora),
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(aurora.contract.account_id.as_str(), &wnear, &aurora),
            FT_TRANSFER_AMOUNT - FT_EXIT_AMOUNT
        );
        assert_eq!(
            erc20_balance(&wnear_erc20, ft_owner_address, &aurora),
            (FT_TRANSFER_AMOUNT - FT_EXIT_AMOUNT).into()
        );
    }

    #[test]
    fn test_exit_to_near_wnear() {
        // Deploy Aurora; deploy wnear; bridge wnear to ERC-20 on Aurora
        let TestExitToNearContext {
            ft_owner,
            ft_owner_address,
            aurora,
            wnear,
            wnear_erc20,
            ..
        } = test_exit_to_near_common();

        let ft_owner_balance = get_account_balance(ft_owner.account_id.as_str(), &aurora);

        // Call exit function on ERC-20; observe ERC-20 burned + wnear tokens transferred
        let result = exit_to_near(
            &ft_owner,
            ft_owner.account_id.as_str(),
            FT_EXIT_AMOUNT,
            &wnear_erc20,
            &aurora,
        );
        let total_tokens_burnt: u128 = result
            .promise_results()
            .iter()
            .map(|r| r.as_ref().unwrap().outcome().tokens_burnt)
            .sum();

        // Check that there were no near tokens transferred to `ft_owner`
        assert_eq!(
            get_account_balance(ft_owner.account_id.as_str(), &aurora),
            ft_owner_balance - total_tokens_burnt
        );

        // Check wnear balances
        assert_eq!(
            nep_141_balance_of(ft_owner.account_id.as_str(), &wnear, &aurora),
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT + FT_EXIT_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(aurora.contract.account_id.as_str(), &wnear, &aurora),
            FT_TRANSFER_AMOUNT - FT_EXIT_AMOUNT
        );
        assert_eq!(
            erc20_balance(&wnear_erc20, ft_owner_address, &aurora),
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
            ..
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
        let exit_account_id = "any.near";

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
            nep_141_balance_of(
                aurora.contract.account_id.as_str(),
                &aurora.contract,
                &aurora,
            ),
            u128::from(INITIAL_ETH_BALANCE - ETH_EXIT_AMOUNT)
        );
        assert_eq!(
            nep_141_balance_of(exit_account_id, &aurora.contract, &aurora),
            u128::from(ETH_EXIT_AMOUNT)
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
            "amount": format!("{INITIAL_ETH_BALANCE}"),
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
        assert_eq!(
            eth_balance_of(signer_address, &aurora),
            Wei::new_u64(INITIAL_ETH_BALANCE)
        );
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

        assert_eq!(
            nep_141_balance_of(
                aurora.contract.account_id.as_str(),
                &aurora.contract,
                &aurora,
            ),
            u128::from(INITIAL_ETH_BALANCE)
        );
        assert_eq!(
            eth_balance_of(signer_address, &aurora),
            Wei::new_u64(INITIAL_ETH_BALANCE)
        );

        // deploy contract with simple exit to near method
        let constructor = TesterConstructor::load();
        let deploy_data = constructor.deploy(0, Address::zero()).data;
        let submit_result = match aurora.call("deploy_code", &deploy_data).status() {
            near_sdk_sim::transaction::ExecutionStatus::SuccessValue(bytes) => {
                SubmitResult::try_from_slice(&bytes).unwrap()
            }
            other => panic!("Unexpected status {other:?}"),
        };
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

        // 3. Deploy wnear and set wnear address
        let wnear = crate::tests::xcc::deploy_wnear(&aurora);
        let wnear_erc20 = deploy_erc20_from_nep_141(&wnear, &aurora);
        aurora
            .call(
                "factory_set_wnear_address",
                wnear_erc20.0.address.as_bytes(),
            )
            .assert_success();

        // 4. Transfer wnear to `ft_owner` and bridge it to aurora
        transfer_nep_141(
            &wnear,
            &aurora.user,
            ft_owner.account_id().as_str(),
            FT_TOTAL_SUPPLY,
        );

        transfer_nep_141_to_erc_20(
            &wnear,
            &wnear_erc20,
            &ft_owner,
            ft_owner_address,
            FT_TRANSFER_AMOUNT,
            &aurora,
        );

        assert_eq!(
            nep_141_balance_of(ft_owner.account_id.as_str(), &wnear, &aurora),
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(aurora.contract.account_id.as_str(), &wnear, &aurora),
            FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            erc20_balance(&wnear_erc20, ft_owner_address, &aurora),
            FT_TRANSFER_AMOUNT.into()
        );

        // 5. Deploy NEP-141
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

        // 6. Deploy ERC-20 from NEP-141 and bridge value to Aurora
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
            wnear,
            wnear_erc20,
        }
    }

    fn get_account_balance(account: &str, aurora: &AuroraAccount) -> u128 {
        aurora
            .user
            .borrow_runtime()
            .view_account(account)
            .unwrap()
            .amount()
    }

    fn exit_to_near(
        source: &UserAccount,
        dest: &str,
        amount: u128,
        erc20: &ERC20,
        aurora: &AuroraAccount,
    ) -> ExecutionResult {
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
        let result = source.call(
            aurora.contract.account_id(),
            "call",
            &call_args.try_to_vec().unwrap(),
            near_sdk_sim::DEFAULT_GAS,
            0,
        );
        result.assert_success();
        result
    }

    pub fn transfer_nep_141(nep_141: &UserAccount, source: &UserAccount, dest: &str, amount: u128) {
        source
            .call(
                nep_141.account_id(),
                "storage_deposit",
                json!({
                    "account_id": dest,
                })
                .to_string()
                .as_bytes(),
                near_sdk_sim::DEFAULT_GAS,
                near_sdk_sim::STORAGE_AMOUNT,
            )
            .assert_success();

        source
            .call(
                nep_141.account_id(),
                "ft_transfer",
                format!(
                    r#"{{"receiver_id": "{}", "amount": "{}", "memo": null}}"#,
                    dest, amount,
                )
                .as_bytes(),
                near_sdk_sim::DEFAULT_GAS,
                1,
            )
            .assert_success();
    }

    pub fn transfer_nep_141_to_erc_20(
        nep_141: &UserAccount,
        erc20: &ERC20,
        source: &UserAccount,
        dest: Address,
        amount: u128,
        aurora: &AuroraAccount,
    ) {
        let transfer_args = json!({
            "receiver_id": aurora.contract.account_id.as_str(),
            "amount": format!("{amount}"),
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
        let submit_result = match result.status() {
            near_sdk_sim::transaction::ExecutionStatus::SuccessValue(bytes) => {
                SubmitResult::try_from_slice(&bytes).unwrap()
            }
            other => panic!("Unexpected status {other:?}"),
        };
        U256::from_big_endian(&test_utils::unwrap_success(submit_result))
    }

    pub fn deploy_erc20_from_nep_141(nep_141: &UserAccount, aurora: &AuroraAccount) -> ERC20 {
        let args = DeployErc20TokenArgs {
            nep141: nep_141.account_id().as_str().parse().unwrap(),
        };
        let result = aurora.call("deploy_erc20_token", &args.try_to_vec().unwrap());
        let addr_bytes: Vec<u8> = result.unwrap_borsh();
        let address = Address::try_from_slice(&addr_bytes).unwrap();
        let abi = ERC20Constructor::load().0.abi;
        ERC20(test_utils::solidity::DeployedContract { abi, address })
    }

    pub fn nep_141_balance_of(
        account_id: &str,
        nep_141: &UserAccount,
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
    /// `https://github.com/near/near-sdk-rs/blob/master/examples/fungible-token/ft/src/lib.rs`
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
            "total_supply": format!("{amount}"),
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

    struct TestExitToNearContext {
        ft_owner: UserAccount,
        ft_owner_address: Address,
        nep_141: UserAccount,
        erc20: ERC20,
        aurora: AuroraAccount,
        wnear: UserAccount,
        wnear_erc20: ERC20,
    }

    struct TestExitToNearEthContext {
        signer: test_utils::Signer,
        signer_address: Address,
        chain_id: u64,
        tester_address: Address,
        aurora: AuroraAccount,
    }
}

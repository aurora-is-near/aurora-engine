use crate::prelude::{Address, Balance, Wei, WeiU256, U256};
use crate::utils::{self, create_eth_transaction, AuroraRunner, DEFAULT_AURORA_ACCOUNT_ID};
use aurora_engine::engine::EngineError;
use aurora_engine::parameters::{CallArgs, FunctionCallArgsV2};
use aurora_engine_transactions::legacy::LegacyEthSignedTransaction;
use aurora_engine_types::borsh::BorshDeserialize;
use aurora_engine_types::parameters::engine::{SubmitResult, TransactionStatus};
use ethabi::Token;
use libsecp256k1::SecretKey;
use near_vm_runner::logic::VMOutcome;
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
    utils::address_from_secret_key(&source_account)
}

pub struct EthereumAddress {
    pub secret_key: SecretKey,
    pub address: Address,
}

impl AuroraRunner {
    pub fn new() -> Self {
        utils::deploy_runner()
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
            borsh::to_vec(&CallArgs::V2(FunctionCallArgsV2 {
                contract,
                value: WeiU256::default(),
                input,
            }))
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
            .make_call(
                "deploy_erc20_token",
                DEFAULT_AURORA_ACCOUNT_ID,
                borsh::to_vec(&nep141).unwrap(),
            )
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
        let source_address = utils::address_from_secret_key(&source_account);
        self.create_address(source_address, INITIAL_BALANCE, INITIAL_NONCE.into());
        EthereumAddress {
            secret_key: source_account,
            address: source_address,
        }
    }

    pub fn balance_of(&mut self, token: Address, target: Address, origin: &str) -> U256 {
        let input = build_input(
            "balanceOf(address)",
            &[Token::Address(target.raw().0.into())],
        );
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
                Token::Address(target.raw().0.into()),
                Token::Uint(amount.into()),
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
                Token::Address(receiver.raw().0.into()),
                Token::Uint(amount.into()),
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
            borsh::to_vec(&relayer_address).unwrap(),
        )
    }

    pub fn factory_set_wnear_address(
        &mut self,
        wnear_address: Address,
    ) -> Result<VMOutcome, EngineError> {
        self.make_call(
            "factory_set_wnear_address",
            DEFAULT_AURORA_ACCOUNT_ID,
            borsh::to_vec(&wnear_address).unwrap(),
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
    let balance = runner.balance_of(token, address, DEFAULT_AURORA_ACCOUNT_ID);
    assert_eq!(balance, U256::from(0));
    let amount = 10;
    let _result = runner.mint(token, address, amount, DEFAULT_AURORA_ACCOUNT_ID);
    let balance = runner.balance_of(token, address, DEFAULT_AURORA_ACCOUNT_ID);
    assert_eq!(balance, U256::from(amount));
}

#[test]
fn test_mint_not_admin() {
    let mut runner = AuroraRunner::new();
    let token = runner.deploy_erc20_token("tt.testnet");
    let address = runner.create_account().address;
    let balance = runner.balance_of(token, address, DEFAULT_AURORA_ACCOUNT_ID);
    assert_eq!(balance, U256::from(0));
    let amount = 10;
    runner.mint(token, address, amount, "not_admin").unwrap();
    let balance = runner.balance_of(token, address, DEFAULT_AURORA_ACCOUNT_ID);
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

    let balance = runner.balance_of(token, recipient, DEFAULT_AURORA_ACCOUNT_ID);
    assert_eq!(balance, U256::from(0));

    let res = runner.ft_on_transfer(nep141, alice, alice, amount, &recipient.encode());
    // Transaction should succeed so return amount is 0
    assert_eq!(res, "\"0\"");

    let balance = runner.balance_of(token, recipient, DEFAULT_AURORA_ACCOUNT_ID);
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

    let balance = runner.balance_of(token, recipient, DEFAULT_AURORA_ACCOUNT_ID);
    assert_eq!(balance, U256::from(0));

    let fee_encoded = U256::from(fee).to_big_endian();

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

    let balance = runner.balance_of(token, recipient, DEFAULT_AURORA_ACCOUNT_ID);
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
        runner.balance_of(token, peer0.address, DEFAULT_AURORA_ACCOUNT_ID),
        U256::zero()
    );
    assert_eq!(
        runner.balance_of(token, peer1.address, DEFAULT_AURORA_ACCOUNT_ID),
        U256::zero()
    );

    runner
        .mint(token, peer0.address, to_mint, DEFAULT_AURORA_ACCOUNT_ID)
        .unwrap();

    assert_eq!(
        runner.balance_of(token, peer0.address, DEFAULT_AURORA_ACCOUNT_ID),
        U256::from(to_mint)
    );

    runner
        .transfer_erc20(
            token,
            peer0.secret_key,
            peer1.address,
            to_transfer,
            DEFAULT_AURORA_ACCOUNT_ID,
        )
        .unwrap();
    assert_eq!(
        runner.balance_of(token, peer0.address, DEFAULT_AURORA_ACCOUNT_ID),
        U256::from(to_mint - to_transfer)
    );

    assert_eq!(
        runner.balance_of(token, peer1.address, DEFAULT_AURORA_ACCOUNT_ID),
        U256::from(to_transfer)
    );
}

pub mod workspace {
    use super::build_input;
    use crate::prelude::{Address, Wei, WeiU256, U256};
    use crate::utils;
    use crate::utils::solidity::erc20::ERC20;
    use crate::utils::solidity::exit_precompile::TesterConstructor;
    use crate::utils::workspace::{
        create_sub_account, deploy_engine, deploy_erc20_from_nep_141, deploy_nep_141,
        nep_141_balance_of, transfer_nep_141, transfer_nep_141_to_erc_20,
    };
    use aurora_engine::parameters::{CallArgs, FunctionCallArgsV2};
    use aurora_engine::proof::Proof;
    use aurora_engine_types::parameters::engine::TransactionStatus;
    use aurora_engine_workspace::account::Account;
    use aurora_engine_workspace::types::{ExecutionFinalResult, NearToken};
    use aurora_engine_workspace::{EngineContract, RawContract};

    const BALANCE: NearToken = NearToken::from_near(50);
    const FT_TOTAL_SUPPLY: u128 = 1_000_000;
    const FT_TRANSFER_AMOUNT: u128 = 300_000;
    const FT_EXIT_AMOUNT: u128 = 100_000;
    const FT_ACCOUNT: &str = "test_token";
    const INITIAL_ETH_BALANCE: u64 = 777_777_777;
    const ETH_EXIT_AMOUNT: u64 = 111_111_111;
    const ETH_CUSTODIAN_ADDRESS: &str = "096de9c2b8a5b8c22cee3289b101f6960d68e51e";
    #[cfg(not(feature = "ext-connector"))]
    const ONE_YOCTO: NearToken = NearToken::from_yoctonear(1);

    #[tokio::test]
    async fn test_ghsa_5c82_x4m4_hcj6_exploit() {
        let TestExitToNearEthContext {
            mut signer,
            signer_address,
            chain_id,
            tester_address: _,
            aurora,
        } = test_exit_to_near_eth_common().await.unwrap();

        let constructor = utils::solidity::ContractConstructor::force_compile(
            "src/tests/res",
            "target/solidity_build",
            "exploit.sol",
            "Exploit",
        );
        let nonce = signer.use_nonce().into();
        let deploy_tx = constructor.deploy_without_constructor(nonce);
        let signed_tx = utils::sign_transaction(deploy_tx, Some(chain_id), &signer.secret_key);
        let deploy_result = aurora
            .submit(rlp::encode(&signed_tx).to_vec())
            .max_gas()
            .transact()
            .await
            .unwrap();
        let contract_address =
            if let TransactionStatus::Succeed(bytes) = &deploy_result.value().status {
                Address::try_from_slice(bytes).unwrap()
            } else {
                panic!("Unknown result: {deploy_result:?}");
            };
        let contract = constructor.deployed_at(contract_address);
        let nonce = signer.use_nonce().into();
        let hacker_account = "hacker.near";
        let mut exploit_tx = contract.call_method_with_args(
            "exploit",
            &[ethabi::Token::Bytes(hacker_account.as_bytes().to_vec())],
            nonce,
        );
        exploit_tx.value = Wei::new_u64(ETH_EXIT_AMOUNT);
        let signed_tx = utils::sign_transaction(exploit_tx, Some(chain_id), &signer.secret_key);
        let result = aurora
            .submit(rlp::encode(&signed_tx).to_vec())
            .max_gas()
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());

        // check balances -- Hacker does not steal any funds!
        assert_eq!(
            nep_141_balance_of(aurora.as_raw_contract(), &aurora.id()).await,
            u128::from(INITIAL_ETH_BALANCE)
        );
        assert_eq!(
            nep_141_balance_of(aurora.as_raw_contract(), &hacker_account.parse().unwrap()).await,
            0
        );
        assert_eq!(
            eth_balance_of(signer_address, &aurora).await,
            Wei::new_u64(INITIAL_ETH_BALANCE)
        );
    }

    #[tokio::test]
    async fn test_exit_to_near() {
        // Deploy Aurora; deploy NEP-141; bridge NEP-141 to ERC-20 on Aurora
        let TestExitToNearContext {
            ft_owner,
            ft_owner_address,
            nep_141,
            erc20,
            aurora,
            ..
        } = test_exit_to_near_common().await.unwrap();

        // Call exit function on ERC-20; observe ERC-20 burned + NEP-141 transferred
        exit_to_near(
            &ft_owner,
            ft_owner.id().as_ref(),
            FT_EXIT_AMOUNT,
            &erc20,
            &aurora,
        )
        .await
        .unwrap();

        assert_eq!(
            nep_141_balance_of(&nep_141, &ft_owner.id()).await,
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT + FT_EXIT_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(&nep_141, &aurora.id()).await,
            FT_TRANSFER_AMOUNT - FT_EXIT_AMOUNT
        );
        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora).await,
            (FT_TRANSFER_AMOUNT - FT_EXIT_AMOUNT).into()
        );
    }

    #[tokio::test]
    async fn test_exit_to_near_wnear_unwrapped() {
        // Deploy Aurora; deploy wnear; bridge wnear to ERC-20 on Aurora
        let TestExitToNearContext {
            ft_owner,
            ft_owner_address,
            aurora,
            wnear,
            wnear_erc20,
            ..
        } = test_exit_to_near_common().await.unwrap();

        let ft_owner_balance = aurora.node.get_balance(&ft_owner.id()).await.unwrap();

        // Call exit function on ERC-20; observe ERC-20 burned + near tokens transferred
        let result = exit_to_near(
            &ft_owner,
            &format!("{}:unwrap", ft_owner.id().as_ref()),
            FT_EXIT_AMOUNT,
            &wnear_erc20,
            &aurora,
        )
        .await;
        let total_tokens_burnt: u128 = result
            .outcomes()
            .iter()
            .map(|o| o.tokens_burnt.as_yoctonear())
            .sum();

        // We need to skip at least 1 block before checking ft_owner's balance, because the refund
        // receipt is executed in the next(skipped) block, because the transaction broadcasts with
        // default `wait_until` parameter, which is [ExecutedOptimistic](https://github.com/near/nearcore/blob/master/core/primitives/src/views.rs#L1743)
        // since nearcore 1.39.0.
        aurora.node.skip_blocks(1).await.unwrap();

        // Check that the wnear tokens are properly unwrapped and transferred to `ft_owner`
        assert_eq!(
            aurora.node.get_balance(&ft_owner.id()).await.unwrap(),
            ft_owner_balance - total_tokens_burnt + FT_EXIT_AMOUNT
        );

        // Check wnear balances
        assert_eq!(
            nep_141_balance_of(&wnear, &ft_owner.id()).await,
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(&wnear, &aurora.id()).await,
            FT_TRANSFER_AMOUNT - FT_EXIT_AMOUNT
        );
        assert_eq!(
            erc20_balance(&wnear_erc20, ft_owner_address, &aurora).await,
            (FT_TRANSFER_AMOUNT - FT_EXIT_AMOUNT).into()
        );
    }

    #[tokio::test]
    async fn test_exit_to_near_wnear() {
        // Deploy Aurora; deploy wnear; bridge wnear to ERC-20 on Aurora
        let TestExitToNearContext {
            ft_owner,
            ft_owner_address,
            aurora,
            wnear,
            wnear_erc20,
            ..
        } = test_exit_to_near_common().await.unwrap();

        let ft_owner_balance = aurora.node.get_balance(&ft_owner.id()).await.unwrap();

        // Call exit function on ERC-20; observe ERC-20 burned + wnear tokens transferred
        let result = exit_to_near(
            &ft_owner,
            ft_owner.id().as_ref(),
            FT_EXIT_AMOUNT,
            &wnear_erc20,
            &aurora,
        )
        .await;
        let total_tokens_burnt: u128 = result
            .outcomes()
            .iter()
            .map(|o| o.tokens_burnt.as_yoctonear())
            .sum();

        // We need to skip at least 1 block before checking ft_owner's balance, because the refund
        // receipt is executed in the next(skipped) block, because the transaction broadcasts with
        // default `wait_until` parameter, which is [ExecutedOptimistic](https://github.com/near/nearcore/blob/master/core/primitives/src/views.rs#L1743)
        // since nearcore 1.39.0.
        aurora.node.skip_blocks(1).await.unwrap();

        // Check that there were no near tokens transferred to `ft_owner`
        assert_eq!(
            aurora.node.get_balance(&ft_owner.id()).await.unwrap(),
            ft_owner_balance - total_tokens_burnt
        );

        // Check wnear balances
        assert_eq!(
            nep_141_balance_of(&wnear, &ft_owner.id()).await,
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT + FT_EXIT_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(&wnear, &aurora.id()).await,
            FT_TRANSFER_AMOUNT - FT_EXIT_AMOUNT
        );
        assert_eq!(
            erc20_balance(&wnear_erc20, ft_owner_address, &aurora).await,
            (FT_TRANSFER_AMOUNT - FT_EXIT_AMOUNT).into()
        );
    }

    #[tokio::test]
    async fn test_exit_to_near_refund() {
        // Deploy Aurora; deploy NEP-141; bridge NEP-141 to ERC-20 on Aurora
        let TestExitToNearContext {
            ft_owner,
            ft_owner_address,
            nep_141,
            erc20,
            aurora,
            ..
        } = test_exit_to_near_common().await.unwrap();

        // Call exit on ERC-20; ft_transfer promise fails; expect refund on Aurora;
        exit_to_near(
            &ft_owner,
            // The ft_transfer will fail because this account is not registered with the NEP-141
            "unregistered.near",
            FT_EXIT_AMOUNT,
            &erc20,
            &aurora,
        )
        .await
        .unwrap();

        assert_eq!(
            nep_141_balance_of(&nep_141, &ft_owner.id()).await,
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(&nep_141, &aurora.id()).await,
            FT_TRANSFER_AMOUNT
        );

        #[cfg(feature = "error_refund")]
        let balance = FT_TRANSFER_AMOUNT.into();
        // If the refund feature is not enabled then there is no refund in the EVM
        #[cfg(not(feature = "error_refund"))]
        let balance = (FT_TRANSFER_AMOUNT - FT_EXIT_AMOUNT).into();

        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora).await,
            balance
        );
    }

    #[tokio::test]
    async fn test_exit_to_near_eth() {
        // Same test as above, but exit ETH instead of a bridged NEP-141
        let TestExitToNearEthContext {
            signer,
            signer_address,
            chain_id,
            tester_address,
            aurora,
        } = test_exit_to_near_eth_common().await.unwrap();
        let exit_account_id = "any.near";

        // call exit to near
        let input = build_input(
            "withdrawEthToNear(bytes)",
            &[ethabi::Token::Bytes(exit_account_id.as_bytes().to_vec())],
        );
        let tx = utils::create_eth_transaction(
            Some(tester_address),
            Wei::new_u64(ETH_EXIT_AMOUNT),
            input,
            Some(chain_id),
            &signer.secret_key,
        );
        let result = aurora
            .submit(rlp::encode(&tx).to_vec())
            .max_gas()
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());

        // check balances
        assert_eq!(
            nep_141_balance_of(aurora.as_raw_contract(), &aurora.id()).await,
            u128::from(INITIAL_ETH_BALANCE - ETH_EXIT_AMOUNT)
        );

        assert_eq!(
            nep_141_balance_of(aurora.as_raw_contract(), &exit_account_id.parse().unwrap()).await,
            ETH_EXIT_AMOUNT.into()
        );

        assert_eq!(
            eth_balance_of(signer_address, &aurora).await,
            Wei::new_u64(INITIAL_ETH_BALANCE - ETH_EXIT_AMOUNT)
        );
    }

    #[tokio::test]
    async fn test_exit_to_near_eth_refund() {
        // Test the case where the ft_transfer promise from the exit call fails;
        // ensure ETH is refunded.

        let TestExitToNearEthContext {
            signer,
            signer_address,
            chain_id,
            tester_address,
            aurora,
        } = test_exit_to_near_eth_common().await.unwrap();
        let exit_account_id = "any.near";

        // Make the ft_transfer call fail by draining the Aurora account
        let result = aurora
            .ft_transfer(
                &"tmp.near".parse().unwrap(),
                u128::from(INITIAL_ETH_BALANCE).into(),
                &None,
            )
            .max_gas()
            .deposit(NearToken::from_yoctonear(1))
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());

        // call exit to near
        let input = build_input(
            "withdrawEthToNear(bytes)",
            &[ethabi::Token::Bytes(exit_account_id.as_bytes().to_vec())],
        );
        let tx = utils::create_eth_transaction(
            Some(tester_address),
            Wei::new_u64(ETH_EXIT_AMOUNT),
            input,
            Some(chain_id),
            &signer.secret_key,
        );
        let result = aurora
            .submit(rlp::encode(&tx).to_vec())
            .max_gas()
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());

        // check balances
        assert_eq!(
            nep_141_balance_of(aurora.as_raw_contract(), &exit_account_id.parse().unwrap()).await,
            0
        );

        #[cfg(feature = "error_refund")]
        let expected_balance = Wei::new_u64(INITIAL_ETH_BALANCE);
        // If the refund feature is not enabled then there is no refund in the EVM
        #[cfg(not(feature = "error_refund"))]
        let expected_balance = Wei::new_u64(INITIAL_ETH_BALANCE - ETH_EXIT_AMOUNT);

        assert_eq!(
            eth_balance_of(signer_address, &aurora).await,
            expected_balance
        );
    }

    #[cfg(not(feature = "ext-connector"))]
    #[tokio::test]
    async fn test_ft_balances_of() {
        use aurora_engine::parameters::FungibleTokenMetadata;
        use aurora_engine_types::account_id::AccountId;
        use aurora_engine_types::HashMap;

        let aurora = deploy_engine().await;
        let metadata = FungibleTokenMetadata::default();
        aurora
            .set_eth_connector_contract_data(
                aurora.id(),
                ETH_CUSTODIAN_ADDRESS.to_string(),
                metadata,
            )
            .transact()
            .await
            .unwrap();

        deposit_balance(&aurora).await;

        let balances: HashMap<AccountId, u128> = HashMap::from([
            (AccountId::new("account1").unwrap(), 10),
            (AccountId::new("account2").unwrap(), 20),
            (AccountId::new("account3").unwrap(), 30),
        ]);

        for (account_id, amount) in &balances {
            aurora
                .ft_transfer(account_id, (*amount).into(), &None)
                .deposit(ONE_YOCTO)
                .transact()
                .await
                .unwrap();
            let blanace = aurora.ft_balance_of(account_id).await.unwrap().result;
            assert_eq!(blanace.0, *amount);
        }

        let accounts = balances.keys().cloned().collect();
        let result = aurora.ft_balances_of(&accounts).await.unwrap().result;
        assert_eq!(result, balances);
    }

    #[cfg(not(feature = "ext-connector"))]
    #[tokio::test]
    async fn test_get_bridge_prover() {
        let aurora = deploy_engine().await;
        let prover = aurora.get_bridge_prover().await.unwrap().result;
        assert_eq!(prover.as_ref(), "prover.root");
    }

    #[cfg(not(feature = "ext-connector"))]
    #[tokio::test]
    async fn test_pause_ft_transfer() {
        use aurora_engine::contract_methods::connector::internal::{PAUSE_FT, UNPAUSE_ALL};
        use aurora_engine::parameters::FungibleTokenMetadata;
        use aurora_engine_types::account_id::AccountId;

        use crate::utils::workspace::storage_deposit_nep141;

        let aurora = deploy_engine().await;
        let metadata = FungibleTokenMetadata::default();
        aurora
            .set_eth_connector_contract_data(
                aurora.id(),
                ETH_CUSTODIAN_ADDRESS.to_string(),
                metadata,
            )
            .transact()
            .await
            .unwrap();

        deposit_balance(&aurora).await;

        let recipient_id = AccountId::new("account1").unwrap();
        let transfer_amount = 10;

        // Pause ft transfers
        aurora.set_paused_flags(PAUSE_FT).transact().await.unwrap();
        // Verify that the storage deposit is paused
        let result = storage_deposit_nep141(&aurora.id(), &aurora.root(), recipient_id.as_ref())
            .await
            .unwrap()
            .into_result();
        assert!(result.unwrap_err().to_string().contains("ERR_FT_PAUSED"));
        // Try to transfer tokens
        let result = aurora
            .ft_transfer(&recipient_id, transfer_amount.into(), &None)
            .deposit(ONE_YOCTO)
            .transact()
            .await;
        assert!(result.unwrap_err().to_string().contains("ERR_FT_PAUSED"));
        // Verify that no tokens were transferred
        let blanace = aurora.ft_balance_of(&recipient_id).await.unwrap().result;
        assert_eq!(blanace.0, 0);

        // Unpause ft transfers
        aurora
            .set_paused_flags(UNPAUSE_ALL)
            .transact()
            .await
            .unwrap();
        // Transfer tokens
        aurora
            .ft_transfer(&recipient_id, transfer_amount.into(), &None)
            .deposit(ONE_YOCTO)
            .transact()
            .await
            .unwrap();
        // Verify that the tokens has been transferred
        let blanace = aurora.ft_balance_of(&recipient_id).await.unwrap().result;
        assert_eq!(blanace.0, transfer_amount);
    }

    #[cfg(not(feature = "ext-connector"))]
    #[tokio::test]
    async fn test_pause_ft_transfer_call() {
        use crate::utils::workspace::transfer_call_nep_141;
        use aurora_engine::contract_methods::connector::internal::{PAUSE_FT, UNPAUSE_ALL};
        use aurora_engine::parameters::FungibleTokenMetadata;

        let aurora = deploy_engine().await;
        let metadata = FungibleTokenMetadata::default();
        aurora
            .set_eth_connector_contract_data(
                aurora.id(),
                ETH_CUSTODIAN_ADDRESS.to_string(),
                metadata,
            )
            .transact()
            .await
            .unwrap();

        deposit_balance(&aurora).await;

        let ft_owner = create_sub_account(&aurora.root(), "ft_owner", BALANCE)
            .await
            .unwrap();
        let transfer_amount = 10;
        // Transfer tokens to the `ft_owner` account
        aurora
            .ft_transfer(&ft_owner.id(), transfer_amount.into(), &None)
            .deposit(ONE_YOCTO)
            .transact()
            .await
            .unwrap();
        let blanace = aurora.ft_balance_of(&ft_owner.id()).await.unwrap().result;
        assert_eq!(blanace.0, transfer_amount);

        // Pause ft transfers
        aurora.set_paused_flags(PAUSE_FT).transact().await.unwrap();
        // Try to transfer tokens from `ft_owner` to `aurora` contract by `ft_transfer_call`
        let transfer_call_msg = "000000000000000000000000000000000000dead";
        let result = transfer_call_nep_141(
            &aurora.id(),
            &ft_owner,
            aurora.id().as_ref(),
            transfer_amount,
            transfer_call_msg,
        )
        .await
        .unwrap()
        .into_result();
        assert!(result.unwrap_err().to_string().contains("ERR_FT_PAUSED"));
        let blanace = aurora.ft_balance_of(&ft_owner.id()).await.unwrap().result;
        assert_eq!(blanace.0, transfer_amount);

        // Unpause ft transfers
        aurora
            .set_paused_flags(UNPAUSE_ALL)
            .transact()
            .await
            .unwrap();
        // Transfer tokens from `ft_owner` to `aurora` contract by `ft_transfer_call`
        transfer_call_nep_141(
            &aurora.id(),
            &ft_owner,
            aurora.id().as_ref(),
            transfer_amount,
            transfer_call_msg,
        )
        .await
        .unwrap()
        .into_result()
        .unwrap();
        // Verify that the tokens has been transferred
        let blanace = aurora.ft_balance_of(&ft_owner.id()).await.unwrap().result;
        assert_eq!(blanace.0, 0);
    }

    async fn test_exit_to_near_eth_common() -> anyhow::Result<TestExitToNearEthContext> {
        let aurora = deploy_engine().await;
        let chain_id = aurora.get_chain_id().await?.result.as_u64();
        let signer = utils::Signer::random();
        let signer_address = utils::address_from_secret_key(&signer.secret_key);

        let result = aurora
            .mint_account(signer_address, signer.nonce, INITIAL_ETH_BALANCE)
            .max_gas()
            .transact()
            .await?;
        assert!(result.is_success());

        #[cfg(feature = "ext-connector")]
        deposit_balance(&aurora).await;

        let balance = nep_141_balance_of(aurora.as_raw_contract(), &aurora.id()).await;
        assert_eq!(balance, u128::from(INITIAL_ETH_BALANCE));

        let balance = eth_balance_of(signer_address, &aurora).await;
        assert_eq!(balance, Wei::new_u64(INITIAL_ETH_BALANCE));

        // deploy contract with simple exit to near method
        let constructor = TesterConstructor::load();
        let deploy_data = constructor.deploy(0, Address::zero()).data;
        let result = aurora
            .deploy_code(deploy_data)
            .max_gas()
            .transact()
            .await?
            .into_value();
        let tester_address = if let TransactionStatus::Succeed(bytes) = result.status {
            Address::try_from_slice(&bytes).unwrap()
        } else {
            anyhow::bail!("Wrong submit result: {result:?}");
        };

        Ok(TestExitToNearEthContext {
            signer,
            signer_address,
            chain_id,
            tester_address,
            aurora,
        })
    }

    #[allow(clippy::cognitive_complexity)]
    async fn test_exit_to_near_common() -> anyhow::Result<TestExitToNearContext> {
        // 1. deploy Aurora
        let aurora = deploy_engine().await;

        // 2. Create account
        let ft_owner = create_sub_account(&aurora.root(), "ft_owner", BALANCE).await?;
        let ft_owner_address =
            aurora_engine_sdk::types::near_account_to_evm_address(ft_owner.id().as_bytes());
        let result = aurora
            .mint_account(ft_owner_address, 0u64, INITIAL_ETH_BALANCE)
            .max_gas()
            .transact()
            .await?;
        assert!(result.is_success());

        // 3. Deploy wnear and set wnear address

        let wnear = crate::tests::xcc::workspace::deploy_wnear(&aurora).await?;
        let wnear_erc20 = deploy_erc20_from_nep_141(wnear.id().as_ref(), &aurora).await?;
        aurora
            .factory_set_wnear_address(wnear_erc20.0.address)
            .transact()
            .await?;

        // 4. Transfer wnear to `ft_owner` and bridge it to aurora
        transfer_nep_141(
            &wnear.id(),
            &aurora.root(),
            ft_owner.id().as_ref(),
            FT_TOTAL_SUPPLY,
        )
        .await?;

        transfer_nep_141_to_erc_20(
            &wnear,
            &wnear_erc20,
            &ft_owner,
            ft_owner_address,
            FT_TRANSFER_AMOUNT,
            &aurora,
        )
        .await?;

        assert_eq!(
            nep_141_balance_of(&wnear, &ft_owner.id()).await,
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(&wnear, &aurora.id()).await,
            FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            erc20_balance(&wnear_erc20, ft_owner_address, &aurora).await,
            FT_TRANSFER_AMOUNT.into()
        );

        // 5. Deploy NEP-141
        let nep_141_account = create_sub_account(&aurora.root(), FT_ACCOUNT, BALANCE).await?;

        let nep_141 = deploy_nep_141(&nep_141_account, &ft_owner, FT_TOTAL_SUPPLY, &aurora)
            .await
            .map_err(|e| anyhow::anyhow!("Couldn't deploy NEP-141: {e}"))?;

        assert_eq!(
            nep_141_balance_of(&nep_141, &ft_owner.id()).await,
            FT_TOTAL_SUPPLY
        );

        // 6. Deploy ERC-20 from NEP-141 and bridge value to Aurora
        let erc20 = deploy_erc20_from_nep_141(nep_141.id().as_ref(), &aurora)
            .await
            .map_err(|e| anyhow::anyhow!("Couldn't deploy ERC-20 from NEP-141: {e}"))?;

        transfer_nep_141_to_erc_20(
            &nep_141,
            &erc20,
            &ft_owner,
            Address::from_array(ft_owner_address.raw().0),
            FT_TRANSFER_AMOUNT,
            &aurora,
        )
        .await?;

        assert_eq!(
            nep_141_balance_of(&nep_141, &ft_owner.id()).await,
            FT_TOTAL_SUPPLY - FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            nep_141_balance_of(&nep_141, &aurora.id()).await,
            FT_TRANSFER_AMOUNT
        );
        assert_eq!(
            erc20_balance(&erc20, ft_owner_address, &aurora).await,
            FT_TRANSFER_AMOUNT.into()
        );

        Ok(TestExitToNearContext {
            ft_owner,
            ft_owner_address,
            nep_141,
            erc20,
            aurora,
            wnear,
            wnear_erc20,
        })
    }

    pub async fn exit_to_near(
        source: &Account,
        dest: &str,
        amount: u128,
        erc20: &ERC20,
        aurora: &EngineContract,
    ) -> ExecutionFinalResult {
        let input = build_input(
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
        let result = source
            .call(&aurora.id(), "call")
            .args_borsh(call_args)
            .max_gas()
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());
        result
    }

    async fn eth_balance_of(address: Address, aurora: &EngineContract) -> Wei {
        let result = aurora.get_balance(address).await.unwrap().result;
        Wei::new(result)
    }

    pub async fn erc20_balance(erc20: &ERC20, address: Address, aurora: &EngineContract) -> U256 {
        let balance_tx = erc20.balance_of(address, 0.into());
        let result = aurora
            .call(erc20.0.address, U256::zero(), balance_tx.data)
            .transact()
            .await
            .unwrap();
        assert!(result.is_success());

        match &result.value().status {
            TransactionStatus::Succeed(bytes) => U256::from_big_endian(bytes),
            _ => panic!("Unexpected status {result:?}"),
        }
    }

    async fn deposit_balance(aurora: &EngineContract) {
        let proof = create_test_proof(
            INITIAL_ETH_BALANCE,
            aurora.id().as_ref(),
            ETH_CUSTODIAN_ADDRESS,
        );
        let result = aurora.deposit(proof).max_gas().transact().await.unwrap();
        assert!(result.is_success());
    }

    struct TestExitToNearContext {
        ft_owner: Account,
        ft_owner_address: Address,
        nep_141: RawContract,
        erc20: ERC20,
        aurora: EngineContract,
        wnear: RawContract,
        wnear_erc20: ERC20,
    }

    struct TestExitToNearEthContext {
        signer: utils::Signer,
        signer_address: Address,
        chain_id: u64,
        tester_address: Address,
        aurora: EngineContract,
    }

    fn create_test_proof(
        deposit_amount: u64,
        recipient_id: &str,
        custodian_address: &str,
    ) -> Proof {
        use aurora_engine::contract_methods::connector::deposit_event::{
            DepositedEvent, TokenMessageData, DEPOSITED_EVENT,
        };
        use aurora_engine_types::types::{Fee, NEP141Wei};

        let eth_custodian_address: Address = Address::decode(custodian_address).unwrap();

        let message = recipient_id.to_string();
        let fee: Fee = Fee::new(NEP141Wei::new(0));
        let token_message_data =
            TokenMessageData::parse_event_message_and_prepare_token_message_data(&message).unwrap();

        let deposit_event = DepositedEvent {
            eth_custodian_address,
            sender: Address::zero(),
            token_message_data,
            amount: NEP141Wei::new(deposit_amount.into()),
            fee,
        };

        let event_schema = ethabi::Event {
            name: DEPOSITED_EVENT.into(),
            inputs: DepositedEvent::event_params(),
            anonymous: false,
        };
        let log_entry = aurora_engine_types::parameters::connector::LogEntry {
            address: eth_custodian_address.raw(),
            topics: vec![
                event_schema.signature().0.into(),
                // the sender is not important
                crate::prelude::H256::zero(),
            ],
            data: ethabi::encode(&[
                ethabi::Token::String(message),
                ethabi::Token::Uint(deposit_event.amount.as_u128().into()),
                ethabi::Token::Uint(deposit_event.fee.as_u128().into()),
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
}

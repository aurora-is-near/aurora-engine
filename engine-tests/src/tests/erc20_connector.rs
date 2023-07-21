use crate::prelude::{Address, Balance, Wei, WeiU256, U256};
use crate::utils::{self, create_eth_transaction, AuroraRunner, ORIGIN};
use aurora_engine::engine::EngineError;
use aurora_engine::parameters::{CallArgs, FunctionCallArgsV2};
use aurora_engine_transactions::legacy::LegacyEthSignedTransaction;
use aurora_engine_types::borsh::{BorshDeserialize, BorshSerialize};
use aurora_engine_types::parameters::engine::{SubmitResult, TransactionStatus};
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
        let source_address = utils::address_from_secret_key(&source_account);
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

pub mod workspace {
    use super::build_input;
    use crate::prelude::{Address, Wei, WeiU256, U256};
    use crate::utils;
    use crate::utils::solidity::erc20::ERC20;
    use crate::utils::solidity::exit_precompile::TesterConstructor;
    use crate::utils::workspace::{
        create_sub_account, deploy_engine, deploy_erc20_from_nep_141, deploy_nep_141,
        nep_141_balance_of, transfer_nep_141_to_erc_20,
    };
    use aurora_engine::parameters::{CallArgs, FunctionCallArgsV2};
    use aurora_engine_types::parameters::engine::TransactionStatus;
    use aurora_engine_workspace::account::Account;
    use aurora_engine_workspace::{parse_near, EngineContract, RawContract};

    const FT_TOTAL_SUPPLY: u128 = 1_000_000;
    const FT_TRANSFER_AMOUNT: u128 = 300_000;
    const FT_EXIT_AMOUNT: u128 = 100_000;
    const FT_ACCOUNT: &str = "test_token";
    const INITIAL_ETH_BALANCE: u64 = 777_777_777;
    const ETH_EXIT_AMOUNT: u64 = 111_111_111;

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
        } = test_exit_to_near_common().await.unwrap();

        // Call exit function on ERC-20; observe ERC-20 burned + NEP-141 transferred
        exit_to_near(
            &ft_owner,
            ft_owner.id().as_ref(),
            FT_EXIT_AMOUNT,
            &erc20,
            &aurora,
        )
        .await;

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
    async fn test_exit_to_near_refund() {
        // Deploy Aurora; deploy NEP-141; bridge NEP-141 to ERC-20 on Aurora
        let TestExitToNearContext {
            ft_owner,
            ft_owner_address,
            nep_141,
            erc20,
            aurora,
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
        .await;

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
            u128::from(ETH_EXIT_AMOUNT)
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
                None,
            )
            .max_gas()
            .deposit(1)
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

        let balance = aurora.ft_balance_of(&aurora.id()).await?.result;
        assert_eq!(balance.0, u128::from(INITIAL_ETH_BALANCE));

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

    async fn test_exit_to_near_common() -> anyhow::Result<TestExitToNearContext> {
        // 1. deploy Aurora
        let aurora = deploy_engine().await;

        // 2. Create account
        let ft_owner = create_sub_account(&aurora.root(), "ft_owner", parse_near!("50 N")).await?;
        let ft_owner_address =
            aurora_engine_sdk::types::near_account_to_evm_address(ft_owner.id().as_bytes());
        let result = aurora
            .mint_account(ft_owner_address, 0u64, INITIAL_ETH_BALANCE)
            .max_gas()
            .transact()
            .await?;
        assert!(result.is_success());

        let nep_141_account =
            create_sub_account(&aurora.root(), FT_ACCOUNT, parse_near!("50 N")).await?;
        // 3. Deploy NEP-141
        let nep_141 = deploy_nep_141(&nep_141_account, &ft_owner, FT_TOTAL_SUPPLY, &aurora)
            .await
            .map_err(|e| anyhow::anyhow!("Couldn't deploy NEP-141: {e}"))?;

        assert_eq!(
            nep_141_balance_of(&nep_141, &ft_owner.id()).await,
            FT_TOTAL_SUPPLY
        );

        // 4. Deploy ERC-20 from NEP-141 and bridge value to Aurora
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
        })
    }

    pub async fn exit_to_near(
        source: &Account,
        dest: &str,
        amount: u128,
        erc20: &ERC20,
        aurora: &EngineContract,
    ) {
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

    struct TestExitToNearContext {
        ft_owner: Account,
        ft_owner_address: Address,
        nep_141: RawContract,
        erc20: ERC20,
        aurora: EngineContract,
    }

    struct TestExitToNearEthContext {
        signer: utils::Signer,
        signer_address: Address,
        chain_id: u64,
        tester_address: Address,
        aurora: EngineContract,
    }
}

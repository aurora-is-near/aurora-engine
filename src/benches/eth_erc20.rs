use crate::prelude::{Address, U256};
use crate::transaction::EthTransaction;
use criterion::{BatchSize, BenchmarkId, Criterion};
use secp256k1::SecretKey;
use std::path::{Path, PathBuf};

use crate::test_utils::solidity;
use crate::test_utils::{address_from_secret_key, deploy_evm, sign_transaction, SUBMIT};

const INITIAL_BALANCE: u64 = 1000;
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: u64 = 67;

pub(crate) fn eth_erc20_benchmark(c: &mut Criterion) {
    let mut runner = deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    runner.create_address(
        address_from_secret_key(&source_account),
        INITIAL_BALANCE.into(),
        INITIAL_NONCE.into(),
    );
    let calling_account_id = "some-account.near".to_string();

    // deploy the erc20 contract
    let constructor = ERC20Constructor::load();
    let contract = ERC20(runner.deploy_contract(
        &source_account,
        |c| c.deploy("Benchmarker", "BENCH", INITIAL_NONCE.into()),
        constructor,
    ));

    // create the transaction for minting
    let tx = contract.mint(
        address_from_secret_key(&source_account),
        INITIAL_BALANCE.into(),
        U256::from(INITIAL_NONCE + 1),
    );
    let signed_tx = sign_transaction(tx, Some(runner.chain_id), &source_account);
    let mint_tx_bytes = rlp::encode(&signed_tx).to_vec();

    // create the transaction for transfer
    let dest_address = address_from_secret_key(&SecretKey::random(&mut rng));
    let tx = contract.transfer(
        dest_address,
        TRANSFER_AMOUNT.into(),
        U256::from(INITIAL_NONCE + 2),
    );
    let signed_tx = sign_transaction(tx, Some(runner.chain_id), &source_account);
    let transfer_tx_bytes = rlp::encode(&signed_tx).to_vec();

    let mut group = c.benchmark_group("erc20");
    let mint_id = BenchmarkId::from_parameter("mint");
    let transfer_id = BenchmarkId::from_parameter("transfer");

    // measure mint wall-clock time
    group.bench_function(mint_id, |b| {
        b.iter_batched(
            || {
                (
                    runner.one_shot(),
                    calling_account_id.clone(),
                    mint_tx_bytes.clone(),
                )
            },
            |(r, c, i)| r.call(SUBMIT, c, i),
            BatchSize::SmallInput,
        )
    });

    // Measure mint gas usage; don't use `one_shot` because we want to keep this state change for
    // the next benchmark where we transfer some of the minted tokens.
    let (output, maybe_error) =
        runner.call(SUBMIT, calling_account_id.clone(), mint_tx_bytes.clone());
    assert!(maybe_error.is_none());
    let output = output.unwrap();
    let gas = output.burnt_gas;
    let eth_gas = crate::test_utils::parse_eth_gas(&output);
    // TODO(#45): capture this in a file
    println!("ETH_ERC20_MINT NEAR GAS: {:?}", gas);
    println!("ETH_ERC20_MINT ETH GAS: {:?}", eth_gas);

    // Measure transfer gas usage
    let (output, maybe_err) = runner.one_shot().call(
        SUBMIT,
        calling_account_id.clone(),
        transfer_tx_bytes.clone(),
    );
    assert!(maybe_err.is_none());
    let output = output.unwrap();
    let gas = output.burnt_gas;
    let eth_gas = crate::test_utils::parse_eth_gas(&output);
    // TODO(#45): capture this in a file
    println!("ETH_ERC20_TRANSFER NEAR GAS: {:?}", gas);
    println!("ETH_ERC20_TRANSFER ETH GAS: {:?}", eth_gas);

    // measure transfer wall-clock time
    group.bench_function(transfer_id, |b| {
        b.iter_batched(
            || {
                (
                    runner.one_shot(),
                    calling_account_id.clone(),
                    transfer_tx_bytes.clone(),
                )
            },
            |(r, c, i)| r.call(SUBMIT, c, i),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

struct ERC20Constructor(solidity::ContractConstructor);

struct ERC20(solidity::DeployedContract);

impl From<ERC20Constructor> for solidity::ContractConstructor {
    fn from(c: ERC20Constructor) -> Self {
        c.0
    }
}

impl ERC20Constructor {
    fn load() -> Self {
        Self(solidity::ContractConstructor::compile_from_source(
            Self::download_solidity_sources(),
            Self::solidity_artifacts_path(),
            "token/ERC20/presets/ERC20PresetMinterPauser.sol",
            "ERC20PresetMinterPauser",
        ))
    }

    fn deploy(&self, name: &str, symbol: &str, nonce: U256) -> EthTransaction {
        let data = self
            .0
            .abi
            .constructor()
            .unwrap()
            .encode_input(
                self.0.code.clone(),
                &[
                    ethabi::Token::String(name.to_string()),
                    ethabi::Token::String(symbol.to_string()),
                ],
            )
            .unwrap();
        EthTransaction {
            nonce,
            gas_price: Default::default(),
            gas: Default::default(),
            to: None,
            value: Default::default(),
            data,
        }
    }

    fn download_solidity_sources() -> PathBuf {
        let sources_dir = Path::new("target").join("openzeppelin-contracts");
        let contracts_dir = sources_dir.join("contracts");
        if contracts_dir.exists() {
            contracts_dir
        } else {
            let url = "https://github.com/OpenZeppelin/openzeppelin-contracts";
            let repo = git2::Repository::clone(url, sources_dir).unwrap();
            // repo.path() gives the path of the .git directory, so we need to use the parent
            repo.path().parent().unwrap().join("contracts")
        }
    }

    fn solidity_artifacts_path() -> PathBuf {
        Path::new("target").join("solidity_build")
    }
}

impl ERC20 {
    fn mint(&self, recipient: Address, amount: U256, nonce: U256) -> EthTransaction {
        let data = self
            .0
            .abi
            .function("mint")
            .unwrap()
            .encode_input(&[
                ethabi::Token::Address(recipient),
                ethabi::Token::Uint(amount),
            ])
            .unwrap();
        EthTransaction {
            nonce,
            gas_price: Default::default(),
            gas: Default::default(),
            to: Some(self.0.address),
            value: Default::default(),
            data,
        }
    }

    fn transfer(&self, recipient: Address, amount: U256, nonce: U256) -> EthTransaction {
        let data = self
            .0
            .abi
            .function("transfer")
            .unwrap()
            .encode_input(&[
                ethabi::Token::Address(recipient),
                ethabi::Token::Uint(amount),
            ])
            .unwrap();
        EthTransaction {
            nonce,
            gas_price: Default::default(),
            gas: Default::default(),
            to: Some(self.0.address),
            value: Default::default(),
            data,
        }
    }
}

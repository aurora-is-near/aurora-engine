use aurora_engine::prelude::{Address, U256};
use aurora_engine::transaction::EthTransaction;
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use near_vm_logic::VMOutcome;
use secp256k1::SecretKey;

// We don't use everything in `common`, but that's ok, other benchmarks do
#[allow(dead_code)]
mod common;

use common::{
    address_from_secret_key, deploy_evm, sign_transaction, AuroraRunner, RAW_CALL,
};

const INITIAL_BALANCE: u64 = 1000;
const INITIAL_NONCE: u64 = 0;
const TRANSFER_AMOUNT: u64 = 67;

fn eth_erc20_benchmark(c: &mut Criterion) {
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
    let output = exec_transaction(
        &mut runner,
        constructor.deploy("Benchmarker", "BENCH", U256::zero()),
        &source_account,
    );
    let erc20_address = output.return_data.as_value().unwrap();
    let contract = ERC20 {
        abi: constructor.abi,
        address: Address::from_slice(&erc20_address),
    };

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
            |(r, c, i)| r.call(RAW_CALL, c, i),
            BatchSize::SmallInput,
        )
    });

    // Measure mint gas usage; don't use `one_shot` because we want to keep this state change for
    // the next benchmark where we transfer some of the minted tokens.
    let (output, maybe_error) =
        runner.call(RAW_CALL, calling_account_id.clone(), mint_tx_bytes.clone());
    assert!(maybe_error.is_none());
    let gas = output.unwrap().burnt_gas;
    println!("ETH_ERC20_MINT GAS: {:?}", gas); // TODO: capture this in a file

    // Measure transfer gas usage
    let (output, maybe_err) = runner.one_shot().call(
        RAW_CALL,
        calling_account_id.clone(),
        transfer_tx_bytes.clone(),
    );
    assert!(maybe_err.is_none());
    let gas = output.unwrap().burnt_gas;
    println!("ETH_ERC20_TRANSFER GAS: {:?}", gas); // TODO: capture this in a file

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
            |(r, c, i)| r.call(RAW_CALL, c, i),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

struct ERC20Constructor {
    abi: ethabi::Contract,
    code: Vec<u8>,
}

struct ERC20 {
    abi: ethabi::Contract,
    address: Address,
}

impl ERC20Constructor {
    fn load() -> Self {
        let hex_rep = std::fs::read_to_string("benches/res/ERC20PresetMinterPauser.bin").unwrap();
        let code = hex::decode(&hex_rep).unwrap();
        let reader = std::fs::File::open("benches/res/ERC20PresetMinterPauser.abi").unwrap();
        let abi = ethabi::Contract::load(reader).unwrap();

        Self { abi, code }
    }

    fn deploy(&self, name: &str, symbol: &str, nonce: U256) -> EthTransaction {
        let data = self
            .abi
            .constructor()
            .unwrap()
            .encode_input(
                self.code.clone(),
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
}

impl ERC20 {
    fn mint(&self, recipient: Address, amount: U256, nonce: U256) -> EthTransaction {
        let data = self
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
            to: Some(self.address),
            value: Default::default(),
            data,
        }
    }

    fn transfer(&self, recipient: Address, amount: U256, nonce: U256) -> EthTransaction {
        let data = self
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
            to: Some(self.address),
            value: Default::default(),
            data,
        }
    }
}

fn exec_transaction(
    runner: &mut AuroraRunner,
    tx: EthTransaction,
    account: &SecretKey,
) -> VMOutcome {
    let calling_account_id = "some-account.near".to_string();
    let signed_tx = sign_transaction(tx, Some(runner.chain_id), &account);
    let (output, maybe_err) = runner.call(
        RAW_CALL,
        calling_account_id,
        rlp::encode(&signed_tx).to_vec(),
    );
    assert!(maybe_err.is_none());
    output.unwrap()
}

criterion_group!(benches, eth_erc20_benchmark);
criterion_main!(benches);

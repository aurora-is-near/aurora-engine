use crate::parameters::SubmitResult;
use crate::test_utils;
use crate::types::Wei;
use borsh::BorshDeserialize;
use near_vm_logic::VMOutcome;
use secp256k1::SecretKey;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

const INITIAL_BALANCE: Wei = Wei::new_u64(1_000_000);
const INITIAL_NONCE: u64 = 0;

static DOWNLOAD_ONCE: Once = Once::new();
static COMPILE_ONCE: Once = Once::new();

#[test]
fn test_1_inch_limit_order_deploy() {
    // set up Aurora runner and accounts
    let (mut runner, mut source_account) = initialize();

    let outcome = deploy_1_inch_limit_order_contract(&mut runner, &mut source_account);
    let profile = test_utils::ExecutionProfile::new(&outcome);
    let result: SubmitResult =
        SubmitResult::try_from_slice(&outcome.return_data.as_value().unwrap()).unwrap();

    // more than 4 million Ethereum gas used
    assert!(result.gas_used > 4_000_000);
    // less than 42 NEAR Tgas used
    assert!(profile.all_gas() < 42_000_000_000_000);
    // at least 70% of which is from wasm execution
    assert!(100 * profile.wasm_gas() / profile.all_gas() > 70);
}

fn deploy_1_inch_limit_order_contract(
    runner: &mut test_utils::AuroraRunner,
    signer: &mut test_utils::Signer,
) -> VMOutcome {
    let contract_path = download_and_compile_solidity_sources();
    let constructor =
        test_utils::solidity::ContractConstructor::compile_from_extended_json(contract_path);

    let nonce = signer.use_nonce();
    let deploy_tx = crate::transaction::LegacyEthTransaction {
        nonce: nonce.into(),
        gas_price: Default::default(),
        gas: u64::MAX.into(),
        to: None,
        value: Default::default(),
        data: constructor.code,
    };
    let tx = test_utils::sign_transaction(deploy_tx, Some(runner.chain_id), &signer.secret_key);

    let (outcome, error) = runner.call(
        test_utils::SUBMIT,
        "any_account.near",
        rlp::encode(&tx).to_vec(),
    );
    assert!(error.is_none());
    outcome.unwrap()
}

fn download_and_compile_solidity_sources() -> PathBuf {
    let sources_dir = Path::new("target").join("limit-order-protocol");
    if !sources_dir.exists() {
        // Contracts not already present, so download them (but only once, even
        // if multiple tests running in parallel saw `contracts_dir` does not exist).
        DOWNLOAD_ONCE.call_once(|| {
            let url = "https://github.com/1inch/limit-order-protocol";
            git2::Repository::clone(url, &sources_dir).unwrap();
        });
    }

    COMPILE_ONCE.call_once(|| {
        // install packages
        let status = Command::new("/usr/bin/env")
            .current_dir(&sources_dir)
            .args(["yarn", "install"])
            .status()
            .unwrap();
        assert!(status.success());

        let hardhat = |command: &str| {
            let status = Command::new("/usr/bin/env")
                .current_dir(&sources_dir)
                .args(["node_modules/hardhat/internal/cli/cli.js", command])
                .status()
                .unwrap();
            assert!(status.success());
        };

        // clean and compile
        hardhat("clean");
        hardhat("compile");
    });

    sources_dir.join("artifacts/contracts/LimitOrderProtocol.sol/LimitOrderProtocol.json")
}

fn initialize() -> (test_utils::AuroraRunner, test_utils::Signer) {
    // set up Aurora runner and accounts
    let mut runner = test_utils::deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    let source_address = test_utils::address_from_secret_key(&source_account);
    runner.create_address(source_address, INITIAL_BALANCE, INITIAL_NONCE.into());
    let mut signer = test_utils::Signer::new(source_account);
    signer.nonce = INITIAL_NONCE;

    (runner, signer)
}

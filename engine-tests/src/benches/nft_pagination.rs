use crate::prelude::{Address, Wei, U256};
use crate::test_utils::{self, solidity};
use aurora_engine_transactions::legacy::TransactionLegacy;
use libsecp256k1::SecretKey;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

const INITIAL_BALANCE: Wei = Wei::new_u64(1_000);
const INITIAL_NONCE: u64 = 0;

static DOWNLOAD_ONCE: Once = Once::new();
static COMPILE_ONCE: Once = Once::new();

pub(crate) fn measure_gas_usage(
    total_tokens: usize,
    data_size: usize,
    tokens_per_page: usize,
) -> u64 {
    let (mut runner, mut source_account, dest_address) = initialize_evm();

    let marketplace_constructor = MarketPlaceConstructor::load();
    let nonce = source_account.use_nonce();
    let marketplace = MarketPlace(runner.deploy_contract(
        &source_account.secret_key,
        |c| c.deploy_without_args(nonce.into()),
        marketplace_constructor.0,
    ));

    // mint NFTs
    let data = "0".repeat(data_size);
    for i in 0..total_tokens {
        let result = runner
            .submit_with_signer(&mut source_account, |nonce| {
                marketplace.mint(
                    dest_address,
                    data.clone(),
                    Wei::from_eth(i.into()).unwrap(),
                    nonce,
                )
            })
            .unwrap();
        assert!(result.status.is_ok());
    }

    // show them
    let nonce = source_account.nonce;
    let tx = marketplace.get_page(tokens_per_page, 0, nonce.into());
    let (result, profile) = runner.profiled_view_call(test_utils::as_view_call(tx, dest_address));

    let status = result.unwrap();
    assert!(status.is_ok());
    profile.all_gas()
}

struct MarketPlaceConstructor(solidity::ContractConstructor);

struct MarketPlace(solidity::DeployedContract);

impl MarketPlaceConstructor {
    pub fn load() -> Self {
        let sources_path = Self::download_solidity_sources();
        let compile_artifact = Self::truffle_compile(sources_path);
        Self(solidity::ContractConstructor::compile_from_extended_json(
            compile_artifact,
        ))
    }

    fn truffle_compile<P: AsRef<Path>>(contracts_dir: P) -> PathBuf {
        COMPILE_ONCE.call_once(|| {
            // install npm packages
            let status = Command::new("/usr/bin/env")
                .current_dir(contracts_dir.as_ref())
                .args(["npm", "install"])
                .status()
                .unwrap();
            assert!(status.success());

            // install truffle
            let status = Command::new("/usr/bin/env")
                .current_dir(contracts_dir.as_ref())
                .args(["npm", "install", "--save-dev", "truffle"])
                .status()
                .unwrap();
            assert!(status.success());

            // compile
            let status = Command::new("/usr/bin/env")
                .current_dir(contracts_dir.as_ref())
                .args([
                    "node_modules/truffle/build/cli.bundled.js",
                    "compile",
                    "--all",
                ])
                .status()
                .unwrap();
            assert!(status.success());
        });

        // compile artifacts are saved to this path
        // (specified in truffle config of `NFT-culturas-latinas` repo)
        let artifacts_path =
            std::fs::canonicalize(contracts_dir.as_ref().join("../frontend/src/contracts/"))
                .unwrap();

        artifacts_path.join("MarketPlace.json")
    }

    fn download_solidity_sources() -> PathBuf {
        let sources_dir = Path::new("target").join("NFT-culturas-latinas");
        let contracts_dir = sources_dir.join("blockchain");
        if contracts_dir.exists() {
            contracts_dir
        } else {
            // Contracts not already present, so download them (but only once, even
            // if multiple tests running in parallel saw `contracts_dir` does not exist).
            DOWNLOAD_ONCE.call_once(|| {
                let url = "https://github.com/birchmd/NFT-culturas-latinas.git";
                git2::Repository::clone(url, sources_dir).unwrap();
            });
            contracts_dir
        }
    }
}

impl MarketPlace {
    pub fn mint(
        &self,
        recipient: Address,
        data: String,
        price: Wei,
        nonce: U256,
    ) -> TransactionLegacy {
        self.0.call_method_with_args(
            "minar",
            &[
                ethabi::Token::Address(recipient.raw()),
                ethabi::Token::String(data),
                ethabi::Token::Uint(price.raw()),
            ],
            nonce,
        )
    }

    pub fn get_page(
        &self,
        tokens_per_page: usize,
        page_index: usize,
        nonce: U256,
    ) -> TransactionLegacy {
        self.0.call_method_with_args(
            "obtenerPaginav2",
            &[
                ethabi::Token::Uint(U256::from(tokens_per_page)),
                ethabi::Token::Uint(U256::from(page_index)),
            ],
            nonce,
        )
    }
}

fn initialize_evm() -> (test_utils::AuroraRunner, test_utils::Signer, Address) {
    // set up Aurora runner and accounts
    let mut runner = test_utils::deploy_evm();
    let mut rng = rand::thread_rng();
    let source_account = SecretKey::random(&mut rng);
    let source_address = test_utils::address_from_secret_key(&source_account);
    runner.create_address(source_address, INITIAL_BALANCE, INITIAL_NONCE.into());
    let dest_address = test_utils::address_from_secret_key(&SecretKey::random(&mut rng));
    let mut signer = test_utils::Signer::new(source_account);
    signer.nonce = INITIAL_NONCE;

    runner.wasm_config.limit_config.max_gas_burnt = u64::MAX;

    (runner, signer, dest_address)
}

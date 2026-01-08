use std::time::Duration;

use aurora_engine::engine::EngineError;
use near_primitives_core::gas::Gas;
use near_vm_runner::ContractCode;
use rand::{Rng, SeedableRng};

use super::sanity::initialize_transfer;
use crate::prelude::Wei;
use crate::prelude::{make_address, Address, U256};
use crate::utils::{self, standalone::StandaloneRunner, AuroraRunner, Signer};

const MODEXP_ADDRESS: Address = make_address(0, 5);

#[test]
fn bench_modexp() {
    let mut context = ModExpBenchContext::default();

    // Example with even modulus and very large exponent
    let input = BenchInput {
        base: vec![224, 6, 0, 0, 169, 33, 33, 33, 33, 33, 33, 33, 255, 0, 0, 33],
        exp: vec![35; 216],
        modulus: vec![
            130, 130, 130, 130, 130, 130, 0, 255, 255, 40, 255, 43, 33, 130, 130, 0,
        ],
        num_iters: Some(1_000),
    };
    let result = context.bench(&input);
    assert_eq!(
        result.least(),
        Implementation::Aurora,
        "Aurora not least:\n{result:?}"
    );

    // Example with odd modulus and very small exponent
    let input = BenchInput {
        base: vec![
            64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        exp: vec![3],
        // Modulus is a large, odd number in this test
        modulus: {
            let mut tmp = vec![0; 748];
            let mut rng = rand::rngs::StdRng::seed_from_u64(314159);
            rng.fill(tmp.as_mut_slice());
            *tmp.last_mut().unwrap() = 0x87;
            tmp
        },
        num_iters: Some(1_000),
    };
    let result = context.bench(&input);
    assert_eq!(
        result.least(),
        Implementation::Aurora,
        "Aurora not least:\n{result:?}"
    );

    let input = BenchInput {
        base: vec![
            0x36, 0xAB, 0xD4, 0x52, 0x4E, 0x89, 0xA3, 0x4C, 0x89, 0xC4, 0x20, 0x94, 0x25, 0x47,
            0xE1, 0x2C, 0x7B, 0xE1,
        ],
        exp: vec![0x01, 0x00, 0x00, 0x00, 0x00, 0x05, 0x17, 0xEA, 0x78],
        modulus: vec![
            0x02, 0xF0, 0x75, 0x8C, 0x6A, 0x04, 0x20, 0x09, 0x55, 0xB6, 0x49, 0xC3, 0x57, 0x22,
            0xB8, 0x00, 0x00, 0x00, 0x00,
        ],
        num_iters: Some(1_000),
    };
    let result = context.bench(&input);
    assert_eq!(
        result.least(),
        Implementation::Aurora,
        "Aurora not least:\n{result:?}"
    );

    // TODO: Aurora not least anymore after switching to the nightly-2023-12-15.
    // Typical example with U256-sized inputs.
    let input = BenchInput::random(32);
    let result = context.bench(&input);
    assert_eq!(
        result.least(),
        Implementation::IBig, // FIXME: Should be Aurora.
        "Aurora not least:\n{result:?}"
    );
}

// This test is marked as ignored because it should only be run with `--release`
// specified (it requires the standalone engine to be compiled with an optimized build).
// This test can be run with the command: `cargo make bench-modexp`
#[ignore]
#[test]
fn bench_modexp_standalone() {
    const GAS_LIMIT: u64 = 30_000_000;
    let mut standalone = StandaloneRunner::default();
    let mut signer = Signer::random();

    standalone.init_evm();

    let deploy_contract = |standalone: &mut StandaloneRunner, signer: &mut Signer, path| {
        let contract_code = std::fs::read_to_string(path).unwrap();
        let deploy_tx = utils::create_deploy_transaction(
            hex::decode(contract_code.trim()).unwrap(),
            signer.use_nonce().into(),
        );
        let deploy_result = standalone
            .submit_transaction(&signer.secret_key, deploy_tx)
            .unwrap();
        Address::try_from_slice(&utils::unwrap_success(deploy_result)).unwrap()
    };

    let do_bench = |standalone: &mut StandaloneRunner, signer: &mut Signer, path| {
        let contract_address = deploy_contract(standalone, signer, path);

        let bench_tx = aurora_engine_transactions::legacy::TransactionLegacy {
            nonce: signer.use_nonce().into(),
            gas_price: U256::zero(),
            gas_limit: GAS_LIMIT.into(),
            to: Some(contract_address),
            value: Wei::zero(),
            data: Vec::new(),
        };

        let start = std::time::Instant::now();
        standalone
            .submit_transaction(&signer.secret_key, bench_tx)
            .unwrap();
        let duration = start.elapsed();
        let limit = Duration::from_secs(4);

        assert!(
            duration < limit,
            "{path} failed to run in under {limit:?}, time taken: {duration:?}"
        );
    };

    // These contracts run the modexp precompile in an infinite loop using strategically selecting
    // input that can take a long time to run with some modexp implementations. It should be
    // possible to burn 30M EVM gas (the GAS_LIMIT for these transactions) within 1 second.
    // This test checks this is case for these specially chosen modexp inputs.
    do_bench(
        &mut standalone,
        &mut signer,
        "../etc/tests/modexp-bench/res/evm_contract_1.hex",
    );
    do_bench(
        &mut standalone,
        &mut signer,
        "../etc/tests/modexp-bench/res/evm_contract_2.hex",
    );
}

#[test]
fn test_modexp_oom() {
    let (mut runner, mut signer, _) = initialize_transfer();

    let inputs = [
        // exp_len: i32::MAX
        "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007fffffff0000000000000000000000000000000000000000000000000000000000000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        // exp_len: u32::MAX
        "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ffffffff0000000000000000000000000000000000000000000000000000000000000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        // exp_len: u64::MAX
        "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000c000000000000000000000000000000000000000000000000000000000000000071000000000000ff600000000000000000000000000000000000000000000000",
        // exponent equal to zero
        "000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000001060002",
    ];

    let outputs = [Vec::new(), Vec::new(), Vec::new(), vec![0x01]];

    for (input, output) in inputs.iter().zip(outputs.iter()) {
        check_wasm_modexp(
            &mut runner,
            &mut signer,
            hex::decode(input).unwrap(),
            output,
        );
    }
}

fn check_wasm_modexp(
    runner: &mut AuroraRunner,
    signer: &mut Signer,
    input: Vec<u8>,
    expected_output: &[u8],
) {
    let wasm_result = runner
        .submit_with_signer(signer, |nonce| {
            aurora_engine_transactions::legacy::TransactionLegacy {
                nonce,
                gas_price: U256::zero(),
                gas_limit: u64::MAX.into(),
                to: Some(MODEXP_ADDRESS),
                value: Wei::zero(),
                data: input,
            }
        })
        .unwrap();
    assert_eq!(expected_output, utils::unwrap_success_slice(&wasm_result));
}

/// Input to the modexp call (base, exp, modulus in big-endian bytes).
#[derive(Debug)]
struct BenchInput {
    base: Vec<u8>,
    exp: Vec<u8>,
    modulus: Vec<u8>,
    num_iters: Option<usize>,
}

impl BenchInput {
    /// Generate a random input where the base, exponent and modulus are all the same number of bytes.
    fn random(size: usize) -> Self {
        let mut rng = rand::rngs::StdRng::seed_from_u64(314159);
        let mut make_bytes = || {
            let mut buf = vec![0u8; size];
            rng.fill(buf.as_mut_slice());
            buf
        };

        Self {
            base: make_bytes(),
            exp: make_bytes(),
            modulus: make_bytes(),
            num_iters: None,
        }
    }

    fn to_json(&self) -> String {
        format!(
            r#"{{
                "base": "{}",
                "exp": "{}",
                "modulus": "{}",
                "n_iters": {}
            }}"#,
            hex::encode(&self.base),
            hex::encode(&self.exp),
            hex::encode(&self.modulus),
            self.num_iters
                .map_or_else(|| "null".into(), |n| n.to_string()),
        )
    }
}

#[derive(Debug)]
struct BenchResult {
    /// Amount of Near gas used by Aurora's modexp implementation
    aurora: Gas,
    /// Amount of Near gas used by ibig crate modexp implementation
    ibig: Gas,
    /// Amount of Near gas used by num crate modexp implementation
    num: Result<Gas, EngineError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Implementation {
    Aurora,
    IBig,
    Num,
}

impl BenchResult {
    fn least(&self) -> Implementation {
        let num = self.num.as_ref().copied().unwrap_or(Gas::MAX);

        if self.aurora <= self.ibig && self.aurora <= num {
            Implementation::Aurora
        } else if self.ibig <= self.aurora && self.ibig <= num {
            Implementation::IBig
        } else {
            Implementation::Num
        }
    }
}

struct ModExpBenchContext {
    inner: AuroraRunner,
}

impl ModExpBenchContext {
    fn bench(&mut self, input: &BenchInput) -> BenchResult {
        let input = input.to_json().into_bytes();
        let parse_output = |bytes: &[u8]| -> Vec<u8> {
            let n = bytes.len();
            let parsed = hex::decode(&bytes[1..(n - 1)]).unwrap();
            // remove leading zeros, if any
            let mut tmp = parsed.as_slice();
            while !tmp.is_empty() && tmp[0] == 0 {
                tmp = &tmp[1..];
            }
            tmp.to_vec()
        };

        let outcome = self.inner.call("modexp", "aurora", input.clone()).unwrap();
        let aurora = outcome.burnt_gas;
        let aurora_result = parse_output(&outcome.return_data.as_value().unwrap());

        let outcome = self
            .inner
            .call("modexp_ibig", "aurora", input.clone())
            .unwrap();
        let ibig = outcome.burnt_gas;
        let ibig_result = parse_output(&outcome.return_data.as_value().unwrap());
        assert_eq!(
            aurora_result, ibig_result,
            "Aurora and ibig responses differed!"
        );

        let maybe_outcome = self.inner.call("modexp_num", "aurora", input);
        let num = maybe_outcome.map(|outcome| outcome.burnt_gas);

        BenchResult { aurora, ibig, num }
    }
}

impl Default for ModExpBenchContext {
    fn default() -> Self {
        let mut inner = AuroraRunner::default();
        let bench_contract_bytes = {
            let base_path = std::path::Path::new("../etc")
                .join("tests")
                .join("modexp-bench");
            let artifact_path = utils::rust::compile(base_path);
            std::fs::read(artifact_path).unwrap()
        };

        // Standalone not relevant here because this is not an Aurora Engine instance
        inner.standalone_runner = None;
        inner.max_gas_burnt(u64::MAX);
        inner.set_code(ContractCode::new(bench_contract_bytes, None));

        Self { inner }
    }
}

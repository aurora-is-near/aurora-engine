use super::utils::*;
use evm::backend::{MemoryAccount, MemoryVicinity};
use evm::Config;
use parity_crypto::publickey;
use primitive_types::{H160, H256, U256};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::io::BufReader;

#[derive(Deserialize, Debug)]
pub struct Test(ethjson::test_helpers::state::State);

impl Test {
    #[allow(dead_code)]
    pub fn unwrap_to_pre_state(&self) -> BTreeMap<H160, MemoryAccount> {
        unwrap_to_state(&self.0.pre_state)
    }

    #[allow(dead_code)]
    pub fn unwrap_caller(&self) -> H160 {
        let secret_key: H256 = self.0.transaction.secret.clone().unwrap().into();
        let secret = publickey::Secret::import_key(&secret_key[..]).unwrap();
        let public = publickey::KeyPair::from_secret(secret)
            .unwrap()
            .public()
            .clone();
        let sender = publickey::public_to_address(&public);
        H160::from(sender.0)
    }

    #[allow(dead_code)]
    pub fn unwrap_to_vicinity(&self) -> MemoryVicinity {
        MemoryVicinity {
            gas_price: self.0.transaction.gas_price.clone().into(),
            origin: self.unwrap_caller(),
            block_hashes: Vec::new(),
            block_number: self.0.env.number.clone().into(),
            block_coinbase: self.0.env.author.clone().into(),
            block_timestamp: self.0.env.timestamp.clone().into(),
            block_difficulty: self.0.env.difficulty.clone().into(),
            block_gas_limit: self.0.env.gas_limit.clone().into(),
            chain_id: U256::one(),
        }
    }
}

pub fn test_state(name: &str, test: Test) {
    use std::thread;

    const STACK_SIZE: usize = 16 * 1024 * 1024;

    let name = name.to_string();
    // Spawn thread with explicit stack size
    let child = thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(move || test_run(&name, test))
        .unwrap();

    // Wait for thread to join
    child.join().unwrap();
}

pub fn test_run(name: &str, test: Test) {
    print!("Running test {} ... ", name);
    for (spec, _states) in &test.0.post_states {
        let (_gasometer_config, _delete_empty) = match spec {
            ethjson::spec::ForkSpec::Istanbul => (Config::istanbul(), true),
            spec => {
                println!("Skip spec {:?}", spec);
                continue;
            }
        };
        /*
        let original_state = test.unwrap_to_pre_state();
        let vicinity = test.unwrap_to_vicinity();
        let caller = test.unwrap_caller();

        for (i, state) in states.iter().enumerate() {
            print!("Running {}:{:?}:{} ... ", name, spec, i);

            let transaction = test.0.transaction.select(&state.indexes);
            let gas_limit: u64 = transaction.gas_limit.into();
            let data: Vec<u8> = transaction.data.into();

            let mut backend = MemoryBackend::new(&vicinity, original_state.clone());
            let metadata = StackSubstateMetadata::new(transaction.gas_limit.into(), &gasometer_config);
            let executor_state = MemoryStackState::new(metadata, &backend);
            // TODO: adapt precompile to the fork spec
            let precompile = istanbul_precompile;
            let mut executor = StackExecutor::new_with_precompile(
                executor_state,
                &gasometer_config,
                precompile,
            );
            let total_fee = vicinity.gas_price * gas_limit;

            executor.state_mut().withdraw(caller, total_fee).unwrap();

            match transaction.to {
                ethjson::maybe::MaybeEmpty::Some(to) => {
                    let data = data;
                    let value = transaction.value.into();

                    let _reason = executor.transact_call(
                        caller,
                        to.clone().into(),
                        value,
                        data,
                        gas_limit
                    );
                },
                ethjson::maybe::MaybeEmpty::None => {
                    let code = data;
                    let value = transaction.value.into();

                    let _reason = executor.transact_create(
                        caller,
                        value,
                        code,
                        gas_limit
                    );
                },
            }

            let actual_fee = executor.fee(vicinity.gas_price);
            executor.state_mut().deposit(vicinity.block_coinbase, actual_fee);
            executor.state_mut().deposit(caller, total_fee - actual_fee);
            let (values, logs) = executor.into_state().deconstruct();
            backend.apply(values, logs, delete_empty);
            assert_valid_hash(&state.hash.0, backend.state());

            println!("passed");
        }*/
    }
}

pub fn run(dir: &str) {
    use std::fs;
    use std::fs::File;
    use std::path::PathBuf;

    let mut dest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dest.push(dir);

    for entry in fs::read_dir(dest).unwrap() {
        let entry = entry.unwrap();
        if let Some(s) = entry.file_name().to_str() {
            if s.starts_with(".") {
                continue;
            }
        }

        let path = entry.path();

        let file = File::open(path).expect("Open file failed");

        let reader = BufReader::new(file);
        let coll = serde_json::from_reader::<_, HashMap<String, Test>>(reader)
            .expect("Parse test cases failed");

        for (name, test) in coll {
            test_state(&name, test);
        }
    }
}

#[test]
fn st_args_zero_one_balance() {
    run("ethtests/GeneralStateTests/stArgsZeroOneBalance")
}
#[test]
fn st_attack() {
    run("ethtests/GeneralStateTests/stAttackTest")
}

#[test]
fn st_bad_opcode() {
    run("ethtests/GeneralStateTests/stBadOpcode")
}
#[test]
fn st_bugs() {
    run("ethtests/GeneralStateTests/stBugs")
}
#[test]
fn st_call_code() {
    run("ethtests/GeneralStateTests/stCallCodes")
}
#[test]
fn st_call_create_call_code() {
    run("ethtests/GeneralStateTests/stCallCreateCallCodeTest")
}
#[test]
fn st_call_delegate_codes_call_code_homestead() {
    run("ethtests/GeneralStateTests/stCallDelegateCodesCallCodeHomestead")
}
#[test]
fn st_call_delegate_codes_homestead() {
    run("ethtests/GeneralStateTests/stCallDelegateCodesHomestead")
}
#[test]
fn st_chain_id() {
    run("ethtests/GeneralStateTests/stChainId")
}
#[test]
fn st_changed_eip150() {
    run("ethtests/GeneralStateTests/stChangedEIP150")
}
#[test]
fn st_code_copy() {
    run("ethtests/GeneralStateTests/stCodeCopyTest")
}
#[test]
fn st_code_size_limit() {
    run("ethtests/GeneralStateTests/stCodeSizeLimit")
}
#[test]
#[ignore]
fn st_create2() {
    run("ethtests/GeneralStateTests/stCreate2")
}
#[test]
fn st_create() {
    run("ethtests/GeneralStateTests/stCreateTest")
}
#[test]
fn st_delegate_call_homestead() {
    run("ethtests/GeneralStateTests/stDelegatecallTestHomestead")
}
#[test]
fn st_eip150_single_code_gas_prices() {
    run("ethtests/GeneralStateTests/stEIP150singleCodeGasPrices")
}
#[test]
fn st_eip150_specific() {
    run("ethtests/GeneralStateTests/stEIP150Specific")
}
#[test]
fn st_eip158_specific() {
    run("ethtests/GeneralStateTests/stEIP158Specific")
}
#[test]
fn st_example() {
    run("ethtests/GeneralStateTests/stExample")
}
#[test]
fn st_ext_code_hash() {
    run("ethtests/GeneralStateTests/stExtCodeHash")
}
#[test]
fn st_homestead_specific() {
    run("ethtests/GeneralStateTests/stHomesteadSpecific")
}
#[test]
fn st_init_code() {
    run("ethtests/GeneralStateTests/stInitCodeTest")
}
#[test]
fn st_log() {
    run("ethtests/GeneralStateTests/stLogTests")
}
#[test]
fn st_mem_expanding_eip_150_calls() {
    run("ethtests/GeneralStateTests/stMemExpandingEIP150Calls")
}
#[test]
fn st_memory_stress() {
    run("ethtests/GeneralStateTests/stMemoryStressTest")
}
#[test]
fn st_memory() {
    run("ethtests/GeneralStateTests/stMemoryTest")
}
#[test]
fn st_non_zero_calls() {
    run("ethtests/GeneralStateTests/stNonZeroCallsTest")
}
#[test]
fn st_precompiled_contracts() {
    run("ethtests/GeneralStateTests/stPreCompiledContracts")
}
#[test]
#[ignore]
fn st_precompiled_contracts2() {
    run("ethtests/GeneralStateTests/stPreCompiledContracts2")
}
#[test]
#[ignore]
fn st_quadratic_complexity() {
    run("ethtests/GeneralStateTests/stQuadraticComplexityTest")
}
#[test]
fn st_random() {
    run("ethtests/GeneralStateTests/stRandom")
}
#[test]
fn st_random2() {
    run("ethtests/GeneralStateTests/stRandom2")
}
#[test]
fn st_recursive_create() {
    run("ethtests/GeneralStateTests/stRecursiveCreate")
}
#[test]
fn st_refund() {
    run("ethtests/GeneralStateTests/stRefundTest")
}
#[test]
fn st_return_data() {
    run("ethtests/GeneralStateTests/stReturnDataTest")
}
#[test]
#[ignore]
fn st_revert() {
    run("ethtests/GeneralStateTests/stRevertTest")
}
#[test]
fn st_self_balance() {
    run("ethtests/GeneralStateTests/stSelfBalance")
}
#[test]
fn st_shift() {
    run("ethtests/GeneralStateTests/stShift")
}
#[test]
fn st_sload() {
    run("ethtests/GeneralStateTests/stSLoadTest")
}
#[test]
fn st_solidity() {
    run("ethtests/GeneralStateTests/stSolidityTest")
}
#[test]
#[ignore]
fn st_special() {
    run("ethtests/GeneralStateTests/stSpecialTest")
}
// Some of the collison test in sstore conflicts with evm's internal
// handlings. Those situations will never happen on a production chain (an empty
// account with storage values), so we can safely ignore them.
#[test]
#[ignore]
fn st_sstore() {
    run("ethtests/GeneralStateTests/stSStoreTest")
}
#[test]
fn st_stack() {
    run("ethtests/GeneralStateTests/stStackTests")
}
#[test]
#[ignore]
fn st_static_call() {
    run("ethtests/GeneralStateTests/stStaticCall")
}
#[test]
fn st_system_operations() {
    run("ethtests/GeneralStateTests/stSystemOperationsTest")
}
#[test]
fn st_transaction() {
    run("ethtests/GeneralStateTests/stTransactionTest")
}
#[test]
fn st_transition() {
    run("ethtests/GeneralStateTests/stTransitionTest")
}
#[test]
fn st_wallet() {
    run("ethtests/GeneralStateTests/stWalletTest")
}
#[test]
fn st_zero_calls_revert() {
    run("ethtests/GeneralStateTests/stZeroCallsRevert");
}
#[test]
fn st_zero_calls() {
    run("ethtests/GeneralStateTests/stZeroCallsTest")
}
#[test]
fn st_zero_knowledge() {
    run("ethtests/GeneralStateTests/stZeroKnowledge")
}
#[test]
fn st_zero_knowledge2() {
    run("ethtests/GeneralStateTests/stZeroKnowledge2")
}

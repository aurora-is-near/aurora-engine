use super::utils::*;
use crate::precompiles::Precompiles;
use evm::backend::ApplyBackend;
use evm::backend::{MemoryAccount, MemoryBackend, MemoryVicinity};
use evm::executor::{MemoryStackState, StackExecutor, StackSubstateMetadata};
use evm::{Config, ExitError};
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

/// Matches the address given to Homestead precompiles.
impl<'backend, 'config, B> evm::executor::Precompiles<MemoryStackState<'backend, 'config, B>>
    for Precompiles
{
    fn run(
        &self,
        address: H160,
        input: &[u8],
        target_gas: Option<u64>,
        context: &evm::Context,
        _state: &mut MemoryStackState<B>,
        is_static: bool,
    ) -> Option<Result<evm::executor::PrecompileOutput, ExitError>> {
        let target_gas = match target_gas {
            Some(t) => t,
            None => return Some(Err(ExitError::OutOfGas)),
        };

        let output = self
            .get_fun(&address)
            .map(|fun| (fun)(input, target_gas, context, is_static));

        output.map(|res| res.map(Into::into))
    }

    fn addresses(&self) -> &[H160] {
        &self.addresses
    }
}

pub fn state_test(name: &str, eth_test: Test) {
    print!("Running test {} ... ", name);
    for (spec, states) in &eth_test.0.post_states {
        let (gasometer_config, delete_empty) = match spec {
            ethjson::spec::ForkSpec::Istanbul => (Config::istanbul(), true),
            spec => {
                println!("Skip spec {:?}", spec);
                continue;
            }
        };

        let original_state = eth_test.unwrap_to_pre_state();
        let vicinity = eth_test.unwrap_to_vicinity();
        let caller = eth_test.unwrap_caller();

        for (i, state) in states.iter().enumerate() {
            print!("Running {}:{:?}:{} ... ", name, spec, i);

            let transaction = eth_test.0.transaction.select(&state.indexes);
            let gas_limit: u64 = transaction.gas_limit.into();
            let data: Vec<u8> = transaction.data.into();

            let mut backend = MemoryBackend::new(&vicinity, original_state.clone());
            let metadata = StackSubstateMetadata::new(gas_limit, &gasometer_config);
            let executor_state = MemoryStackState::new(metadata, &backend);
            let precompile = Precompiles::new_istanbul();

            let total_fee = vicinity.gas_price * gas_limit;
            let mut executor =
                StackExecutor::new_with_precompile(executor_state, &gasometer_config, precompile);
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
                        gas_limit,
                        vec![],
                    );
                }
                ethjson::maybe::MaybeEmpty::None => {
                    let code = data;
                    let value = transaction.value.into();

                    let _reason = executor.transact_create(caller, value, code, gas_limit, vec![]);
                }
            }
            let actual_fee = executor.fee(vicinity.gas_price);
            executor
                .state_mut()
                .deposit(vicinity.block_coinbase, actual_fee);
            executor.state_mut().deposit(caller, total_fee - actual_fee);
            let (values, logs) = executor.into_state().deconstruct();
            backend.apply(values, logs, delete_empty);
            assert_valid_hash(&state.hash.0, backend.state());

            println!("passed");
        }
    }
}

pub fn run(dir: &str) {
    use std::fs;
    use std::fs::File;
    use std::path::PathBuf;

    let mut dest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dest.push("src/tests/eth/ethtests");
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
            state_test(&name, test);
        }
    }
}

#[test]
fn st_args_zero_one_balance() {
    run("GeneralStateTests/stArgsZeroOneBalance")
}
#[test]
fn st_attack() {
    run("GeneralStateTests/stAttackTest")
}
#[test]
fn st_bad_opcode() {
    run("GeneralStateTests/stBadOpcode")
}
#[test]
fn st_bugs() {
    run("GeneralStateTests/stBugs")
}
#[test]
fn st_chain_id() {
    run("GeneralStateTests/stChainId")
}
#[test]
fn st_code_copy() {
    run("GeneralStateTests/stCodeCopyTest")
}
#[test]
fn st_code_size_limit() {
    run("GeneralStateTests/stCodeSizeLimit")
}
#[test]
#[ignore]
fn st_create2() {
    run("GeneralStateTests/stCreate2")
}
#[test]
fn st_create() {
    run("GeneralStateTests/stCreateTest")
}
#[test]
fn st_eip150_single_code_gas_prices() {
    run("GeneralStateTests/stEIP150singleCodeGasPrices")
}
#[test]
fn st_eip150_specific() {
    run("GeneralStateTests/stEIP150Specific")
}
#[test]
fn st_eip158_specific() {
    run("GeneralStateTests/stEIP158Specific")
}
#[test]
fn st_example() {
    run("GeneralStateTests/stExample")
}
#[test]
fn st_ext_code_hash() {
    run("GeneralStateTests/stExtCodeHash")
}
#[test]
fn st_homestead_specific() {
    run("GeneralStateTests/stHomesteadSpecific")
}
#[test]
fn st_init_code() {
    run("GeneralStateTests/stInitCodeTest")
}
#[test]
fn st_log() {
    run("GeneralStateTests/stLogTests")
}
#[test]
fn st_mem_expanding_eip_150_calls() {
    run("GeneralStateTests/stMemExpandingEIP150Calls")
}
#[test]
fn st_memory_stress() {
    run("GeneralStateTests/stMemoryStressTest")
}
#[test]
fn st_memory() {
    run("GeneralStateTests/stMemoryTest")
}
#[test]
fn st_non_zero_calls() {
    run("GeneralStateTests/stNonZeroCallsTest")
}
#[test]
#[ignore]
fn st_precompiled_contracts2() {
    run("GeneralStateTests/stPreCompiledContracts2")
}
#[test]
#[ignore]
fn st_quadratic_complexity() {
    run("GeneralStateTests/stQuadraticComplexityTest")
}
#[test]
fn st_refund() {
    run("GeneralStateTests/stRefundTest")
}
#[test]
#[ignore]
fn st_revert() {
    run("GeneralStateTests/stRevertTest")
}
#[test]
fn st_self_balance() {
    run("GeneralStateTests/stSelfBalance")
}
#[test]
fn st_shift() {
    run("GeneralStateTests/stShift")
}
#[test]
fn st_sload() {
    run("GeneralStateTests/stSLoadTest")
}
#[test]
#[ignore]
fn st_special() {
    run("GeneralStateTests/stSpecialTest")
}
// Some of the collison test in sstore conflicts with evm's internal
// handlings. Those situations will never happen on a production chain (an empty
// account with storage values), so we can safely ignore them.
#[test]
#[ignore]
fn st_sstore() {
    run("GeneralStateTests/stSStoreTest")
}
#[test]
fn st_stack() {
    run("GeneralStateTests/stStackTests")
}
#[test]
#[ignore]
fn st_static_call() {
    run("GeneralStateTests/stStaticCall")
}
#[test]
fn st_transaction() {
    run("GeneralStateTests/stTransactionTest")
}
#[test]
fn st_transition() {
    run("GeneralStateTests/stTransitionTest")
}
#[test]
fn st_wallet() {
    run("GeneralStateTests/stWalletTest")
}
#[test]
fn st_zero_calls_revert() {
    run("GeneralStateTests/stZeroCallsRevert");
}
#[test]
fn st_zero_calls() {
    run("GeneralStateTests/stZeroCallsTest")
}

#[test]
#[ignore]
fn st_call_delegate_codes_call_code_homestead() {
    run("GeneralStateTests/stCallDelegateCodesCallCodeHomestead")
}
#[test]
#[ignore]
fn st_call_delegate_codes_homestead() {
    run("GeneralStateTests/stCallDelegateCodesHomestead")
}
#[test]
#[ignore]
fn st_changed_eip150() {
    run("GeneralStateTests/stChangedEIP150")
}
#[test]
#[ignore]
fn st_random() {
    run("GeneralStateTests/stRandom")
}
#[test]
#[ignore]
fn st_precompiled_contracts() {
    run("GeneralStateTests/stPreCompiledContracts")
}
#[test]
#[ignore]
fn st_zero_knowledge() {
    run("GeneralStateTests/stZeroKnowledge")
}
#[test]
#[ignore]
fn st_zero_knowledge2() {
    run("GeneralStateTests/stZeroKnowledge2")
}
#[test]
#[ignore]
fn st_random2() {
    run("GeneralStateTests/stRandom2")
}
#[test]
#[ignore]
fn st_recursive_create() {
    run("GeneralStateTests/stRecursiveCreate")
}
#[test]
#[ignore]
fn st_return_data() {
    run("GeneralStateTests/stReturnDataTest")
}
#[test]
#[ignore]
fn st_delegate_call_homestead() {
    run("GeneralStateTests/stDelegatecallTestHomestead")
}
#[test]
#[ignore]
fn st_call_create_call_code() {
    run("GeneralStateTests/stCallCreateCallCodeTest")
}
#[test]
#[ignore]
fn st_call_code() {
    run("GeneralStateTests/stCallCodes")
}
#[test]
#[ignore]
fn st_system_operations() {
    run("GeneralStateTests/stSystemOperationsTest")
}
#[test]
#[ignore]
fn st_solidity() {
    run("GeneralStateTests/stSolidityTest")
}

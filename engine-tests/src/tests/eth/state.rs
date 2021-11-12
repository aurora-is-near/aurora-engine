use super::ethcore_builtin;
use super::ethjson::{self, spec::ForkSpec};
use super::utils::*;
use crate::prelude::{H160, H256, U256};
use evm::backend::{ApplyBackend, MemoryAccount, MemoryBackend, MemoryVicinity};
use evm::executor::{
    MemoryStackState, PrecompileFailure, PrecompileFn, PrecompileOutput, StackExecutor,
    StackSubstateMetadata,
};
use evm::{Config, Context, ExitError, ExitSucceed};
use lazy_static::lazy_static;
use parity_crypto::publickey;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::convert::TryInto;

#[derive(Deserialize, Debug)]
pub struct Test(ethjson::test_helpers::state::State);

impl Test {
    pub fn unwrap_to_pre_state(&self) -> BTreeMap<H160, MemoryAccount> {
        unwrap_to_state(&self.0.pre_state)
    }

    pub fn unwrap_caller(&self) -> H160 {
        let secret_key: H256 = self.0.transaction.secret.clone().unwrap().into();
        let secret = publickey::Secret::import_key(&secret_key[..]).unwrap();
        let public = publickey::KeyPair::from_secret(secret)
            .unwrap()
            .public()
            .clone();
        let sender = publickey::public_to_address(&public);

        sender
    }

    pub fn unwrap_to_vicinity(&self, spec: &ForkSpec) -> Option<MemoryVicinity> {
        let block_base_fee_per_gas = self.0.env.block_base_fee_per_gas.0;
        let gas_price = if self.0.transaction.gas_price.0.is_zero() {
            let max_fee_per_gas = self.0.transaction.max_fee_per_gas.0;

            // max_fee_per_gas is only defined for London and later
            if !max_fee_per_gas.is_zero() && spec < &ForkSpec::London {
                return None;
            }

            // Cannot specify a lower fee than the base fee
            if max_fee_per_gas < block_base_fee_per_gas {
                return None;
            }

            let max_priority_fee_per_gas = self.0.transaction.max_priority_fee_per_gas.0;

            // priority fee must be lower than regaular fee
            if max_fee_per_gas < max_priority_fee_per_gas {
                return None;
            }

            let priority_fee_per_gas = std::cmp::min(
                max_priority_fee_per_gas,
                max_fee_per_gas - block_base_fee_per_gas,
            );
            priority_fee_per_gas + block_base_fee_per_gas
        } else {
            self.0.transaction.gas_price.0
        };

        // gas price cannot be lower than base fee
        if gas_price < block_base_fee_per_gas {
            return None;
        }

        Some(MemoryVicinity {
            gas_price,
            origin: self.unwrap_caller(),
            block_hashes: Vec::new(),
            block_number: self.0.env.number.clone().into(),
            block_coinbase: self.0.env.author.clone().into(),
            block_timestamp: self.0.env.timestamp.clone().into(),
            block_difficulty: self.0.env.difficulty.clone().into(),
            block_gas_limit: self.0.env.gas_limit.clone().into(),
            chain_id: U256::one(),
            block_base_fee_per_gas,
        })
    }
}

lazy_static! {
    static ref ISTANBUL_BUILTINS: BTreeMap<H160, ethcore_builtin::Builtin> =
        JsonPrecompile::builtins("./res/istanbul_builtins.json");
}

lazy_static! {
    static ref BERLIN_BUILTINS: BTreeMap<H160, ethcore_builtin::Builtin> =
        JsonPrecompile::builtins("./res/berlin_builtins.json");
}

macro_rules! precompile_entry {
    ($map:expr, $builtins:expr, $index:expr) => {
        let x: fn(
            &[u8],
            Option<u64>,
            &Context,
            bool,
        ) -> Result<PrecompileOutput, PrecompileFailure> =
            |input: &[u8], gas_limit: Option<u64>, _context: &Context, _is_static: bool| {
                let builtin = $builtins.get(&H160::from_low_u64_be($index)).unwrap();
                Self::exec_as_precompile(builtin, input, gas_limit)
            };
        $map.insert(H160::from_low_u64_be($index), x);
    };
}

pub struct JsonPrecompile;

impl JsonPrecompile {
    pub fn precompile(spec: &ForkSpec) -> Option<BTreeMap<H160, PrecompileFn>> {
        match spec {
            ForkSpec::Istanbul => {
                let mut map = BTreeMap::new();
                precompile_entry!(map, ISTANBUL_BUILTINS, 1);
                precompile_entry!(map, ISTANBUL_BUILTINS, 2);
                precompile_entry!(map, ISTANBUL_BUILTINS, 3);
                precompile_entry!(map, ISTANBUL_BUILTINS, 4);
                precompile_entry!(map, ISTANBUL_BUILTINS, 5);
                precompile_entry!(map, ISTANBUL_BUILTINS, 6);
                precompile_entry!(map, ISTANBUL_BUILTINS, 7);
                precompile_entry!(map, ISTANBUL_BUILTINS, 8);
                precompile_entry!(map, ISTANBUL_BUILTINS, 9);
                Some(map)
            }
            ForkSpec::Berlin => {
                let mut map = BTreeMap::new();
                precompile_entry!(map, BERLIN_BUILTINS, 1);
                precompile_entry!(map, BERLIN_BUILTINS, 2);
                precompile_entry!(map, BERLIN_BUILTINS, 3);
                precompile_entry!(map, BERLIN_BUILTINS, 4);
                precompile_entry!(map, BERLIN_BUILTINS, 5);
                precompile_entry!(map, BERLIN_BUILTINS, 6);
                precompile_entry!(map, BERLIN_BUILTINS, 7);
                precompile_entry!(map, BERLIN_BUILTINS, 8);
                precompile_entry!(map, BERLIN_BUILTINS, 9);
                Some(map)
            }
            // precompiles for London and Berlin are the same
            ForkSpec::London => Self::precompile(&ForkSpec::Berlin),
            _ => None,
        }
    }

    fn builtins(spec_path: &str) -> BTreeMap<H160, ethcore_builtin::Builtin> {
        let reader = std::fs::File::open(spec_path).unwrap();
        let builtins: BTreeMap<ethjson::hash::Address, ethjson::spec::builtin::BuiltinCompat> =
            serde_json::from_reader(reader).unwrap();
        builtins
            .into_iter()
            .map(|(address, builtin)| {
                (
                    address.into(),
                    ethjson::spec::Builtin::from(builtin).try_into().unwrap(),
                )
            })
            .collect()
    }

    fn exec_as_precompile(
        builtin: &ethcore_builtin::Builtin,
        input: &[u8],
        gas_limit: Option<u64>,
    ) -> Result<PrecompileOutput, PrecompileFailure> {
        let cost = builtin.cost(input, 0);

        if let Some(target_gas) = gas_limit {
            if cost > U256::from(u64::MAX) || target_gas < cost.as_u64() {
                return Err(PrecompileFailure::Error {
                    exit_status: ExitError::OutOfGas,
                });
            }
        }

        let mut output = Vec::new();
        match builtin.execute(input, &mut parity_bytes::BytesRef::Flexible(&mut output)) {
            Ok(()) => Ok(PrecompileOutput {
                exit_status: ExitSucceed::Stopped,
                output,
                cost: cost.as_u64(),
                logs: Vec::new(),
            }),
            Err(e) => Err(PrecompileFailure::Error {
                exit_status: ExitError::Other(e.into()),
            }),
        }
    }
}

pub fn state_test(name: &str, test: Test) {
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

fn test_run(name: &str, test: Test) {
    for (spec, states) in &test.0.post_states {
        let (gasometer_config, delete_empty) = match spec {
            ethjson::spec::ForkSpec::Istanbul => (Config::istanbul(), true),
            ethjson::spec::ForkSpec::Berlin => (Config::berlin(), true),
            ethjson::spec::ForkSpec::London => (Config::london(), true),
            spec => {
                println!("Skip spec {:?}", spec);
                continue;
            }
        };

        let original_state = test.unwrap_to_pre_state();
        let vicinity = test.unwrap_to_vicinity(spec);
        if vicinity.is_none() {
            // if vicinity could not be computed then the transaction was invalid so we simply
            // check the original state and move on
            assert_valid_hash(&states.first().unwrap().hash.0, &original_state);
            continue;
        }
        let vicinity = vicinity.unwrap();
        let caller = test.unwrap_caller();
        let caller_balance = original_state.get(&caller).unwrap().balance;

        for (i, state) in states.iter().enumerate() {
            print!("Running {}:{:?}:{} ... ", name, spec, i);

            let transaction = test.0.transaction.select(&state.indexes);
            let mut backend = MemoryBackend::new(&vicinity, original_state.clone());

            // Only execute valid transactions
            if let Ok(transaction) = transaction::validate(
                transaction,
                test.0.env.gas_limit.0,
                caller_balance,
                &gasometer_config,
            ) {
                let gas_limit: u64 = transaction.gas_limit.into();
                let data: Vec<u8> = transaction.data.into();
                let metadata =
                    StackSubstateMetadata::new(transaction.gas_limit.into(), &gasometer_config);
                let executor_state = MemoryStackState::new(metadata, &backend);
                let precompile = JsonPrecompile::precompile(spec).unwrap();
                let mut executor = StackExecutor::new_with_precompiles(
                    executor_state,
                    &gasometer_config,
                    &precompile,
                );
                let total_fee = vicinity.gas_price * gas_limit;

                executor.state_mut().withdraw(caller, total_fee).unwrap();

                let access_list = transaction
                    .access_list
                    .into_iter()
                    .map(|(address, keys)| (address.0, keys.into_iter().map(|k| k.0).collect()))
                    .collect();

                match transaction.to {
                    ethjson::maybe::MaybeEmpty::Some(to) => {
                        let data = data;
                        let value = transaction.value.into();

                        let _reason = executor.transact_call(
                            caller,
                            to.into(),
                            value,
                            data,
                            gas_limit,
                            access_list,
                        );
                    }
                    ethjson::maybe::MaybeEmpty::None => {
                        let code = data;
                        let value = transaction.value.into();

                        let _reason =
                            executor.transact_create(caller, value, code, gas_limit, access_list);
                    }
                }

                let actual_fee = executor.fee(vicinity.gas_price);
                let mniner_reward = if let ForkSpec::London = spec {
                    // see EIP-1559
                    let max_priority_fee_per_gas = test.0.transaction.max_priority_fee_per_gas();
                    let max_fee_per_gas = test.0.transaction.max_fee_per_gas();
                    let base_fee_per_gas = vicinity.block_base_fee_per_gas;
                    let priority_fee_per_gas =
                        std::cmp::min(max_priority_fee_per_gas, max_fee_per_gas - base_fee_per_gas);
                    executor.fee(priority_fee_per_gas)
                } else {
                    actual_fee
                };
                executor
                    .state_mut()
                    .deposit(vicinity.block_coinbase, mniner_reward);
                executor.state_mut().deposit(caller, total_fee - actual_fee);
                let (values, logs) = executor.into_state().deconstruct();
                backend.apply(values, logs, delete_empty);
            }

            assert_valid_hash(&state.hash.0, backend.state());

            println!("passed");
        }
    }
}

pub fn run(dir: &str) {
    use std::collections::HashMap;
    use std::fs;
    use std::fs::File;
    use std::io::BufReader;
    use std::path::PathBuf;

    let mut dest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dest.push("./ethtests");
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

/*use super::ethjson;
use super::utils::*;
use crate::prelude::{H160, H256, U256};
use aurora_engine_precompiles::Precompiles;
use evm::backend::ApplyBackend;
use evm::backend::{MemoryAccount, MemoryBackend, MemoryVicinity};
use evm::executor::{MemoryStackState, StackExecutor, StackSubstateMetadata};
use evm::{Config, ExitError};
use parity_crypto::publickey;
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
*/

#[test]
fn st_args_zero_one_balance() {
    run("GeneralStateTests/stArgsZeroOneBalance")
}

/*
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
*/

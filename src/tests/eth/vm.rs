use super::utils::*;
use evm::backend::{ApplyBackend, MemoryAccount, MemoryBackend, MemoryVicinity};
use evm::executor::{MemoryStackState, StackExecutor, StackSubstateMetadata};
use evm::Config;
use primitive_types::{H160, U256};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::io::BufReader;
use std::rc::Rc;

#[derive(Deserialize, Debug)]
pub struct Test(ethjson::vm::Vm);

impl Test {
    pub fn unwrap_to_pre_state(&self) -> BTreeMap<H160, MemoryAccount> {
        unwrap_to_state(&self.0.pre_state)
    }

    pub fn unwrap_to_vicinity(&self) -> MemoryVicinity {
        MemoryVicinity {
            gas_price: self.0.transaction.gas_price.clone().into(),
            origin: self.0.transaction.origin.clone().into(),
            block_hashes: Vec::new(),
            block_number: self.0.env.number.clone().into(),
            block_coinbase: self.0.env.author.clone().into(),
            block_timestamp: self.0.env.timestamp.clone().into(),
            block_difficulty: self.0.env.difficulty.clone().into(),
            block_gas_limit: self.0.env.gas_limit.clone().into(),
            chain_id: U256::zero(),
        }
    }

    pub fn unwrap_to_code(&self) -> Rc<Vec<u8>> {
        Rc::new(self.0.transaction.code.clone().into())
    }

    pub fn unwrap_to_data(&self) -> Rc<Vec<u8>> {
        Rc::new(self.0.transaction.data.clone().into())
    }

    pub fn unwrap_to_context(&self) -> evm::Context {
        evm::Context {
            address: self.0.transaction.address.clone().into(),
            caller: self.0.transaction.sender.clone().into(),
            apparent_value: self.0.transaction.value.clone().into(),
        }
    }

    pub fn unwrap_to_return_value(&self) -> Vec<u8> {
        self.0.output.clone().unwrap().into()
    }

    pub fn unwrap_to_gas_limit(&self) -> u64 {
        self.0.transaction.gas.clone().into()
    }

    pub fn unwrap_to_post_gas(&self) -> u64 {
        self.0.gas_left.clone().unwrap().into()
    }
}

fn vm_test(name: &str, eth_test: Test) {
    print!("Running test {} ... ", name);
    let original_state = eth_test.unwrap_to_pre_state();
    let vicinity = eth_test.unwrap_to_vicinity();
    let config = Config::frontier();
    let mut backend = MemoryBackend::new(&vicinity, original_state);
    let metadata = StackSubstateMetadata::new(eth_test.unwrap_to_gas_limit(), &config);
    let state = MemoryStackState::new(metadata, &backend);
    let mut executor = StackExecutor::new(state, &config);

    let code = eth_test.unwrap_to_code();
    let data = eth_test.unwrap_to_data();
    let context = eth_test.unwrap_to_context();
    let mut runtime = evm::Runtime::new(code, data, context, &config);

    let reason = executor.execute(&mut runtime);
    let gas = executor.gas();
    let (values, logs) = executor.into_state().deconstruct();
    backend.apply(values, logs, false);

    if eth_test.0.output.is_none() {
        print!("{:?}", reason);

        assert!(!reason.is_succeed());
        assert!(eth_test.0.post_state.is_none() && eth_test.0.gas_left.is_none());

        println!("succeed");
    } else {
        let expected_post_gas = eth_test.unwrap_to_post_gas();
        print!("{:?} ", reason);

        assert_eq!(
            runtime.machine().return_value(),
            eth_test.unwrap_to_return_value()
        );
        assert_valid_state(eth_test.0.post_state.as_ref().unwrap(), &backend.state());
        assert_eq!(gas, expected_post_gas);
        println!("succeed");
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
        let path = entry.path();

        let file = File::open(path).expect("Open file failed");

        let reader = BufReader::new(file);
        let coll = serde_json::from_reader::<_, HashMap<String, Test>>(reader)
            .expect("Parse test cases failed");

        for (name, test) in coll {
            vm_test(&name, test);
        }
    }
}

#[test]
fn vm_arithmetic() {
    run("VMTests/vmArithmeticTest");
}

#[test]
fn vm_bitwise_logic() {
    run("VMTests/vmBitwiseLogicOperation");
}

#[test]
fn vm_block_info() {
    run("VMTests/vmBlockInfoTest");
}

#[test]
fn vm_environmental_info() {
    run("VMTests/vmEnvironmentalInfo");
}

#[test]
fn vm_io_and_flow() {
    run("VMTests/vmIOandFlowOperations");
}

#[test]
fn vm_log() {
    run("VMTests/vmLogTest");
}

#[test]
#[ignore]
fn vm_performance() {
    run("VMTests/vmPerformance");
}

#[test]
fn vm_push_dup_swap() {
    run("VMTests/vmPushDupSwapTest");
}

#[test]
fn vm_random() {
    run("VMTests/vmRandomTest");
}

#[test]
fn vm_sha3() {
    run("VMTests/vmSha3Test");
}

#[test]
fn vm_system() {
    run("VMTests/vmSystemOperations");
}

#[test]
fn vm_other() {
    run("VMTests/vmTests");
}

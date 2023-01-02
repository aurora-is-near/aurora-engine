use crate::prelude::Wei;
use crate::test_utils::{self, address_from_hex, ExecutionProfile};
use aurora_engine::parameters::SubmitResult;
use eth_json_test::test_types::general_state_test::GeneralStateTest;
use near_vm_runner::VMError;
use rustc_hex::FromHex;

const INITIAL_NONCE: u64 = 0;

fn hexstr_to_bytes(value: &str) -> Vec<u8> {
    let v = match value.len() {
        0 => vec![],
        2 if value.starts_with("0x") => vec![],
        _ if value.starts_with("0x") && value.len() % 2 == 1 => {
            let v = "0".to_owned() + &value[2..];
            FromHex::from_hex(v.as_str()).unwrap_or_default()
        }
        _ if value.starts_with("0x") => FromHex::from_hex(&value[2..]).unwrap_or_default(),
        _ => FromHex::from_hex(value).unwrap_or_default(),
    };

    v
}

fn initialize_runner(path: String, name: String) -> (test_utils::AuroraRunner, GeneralStateTest) {
    // Get json from Ethereum tests
    let gst_json = GeneralStateTest::new(path, name);

    // set up Aurora runner and accounts
    let mut runner = test_utils::deploy_evm();
    // setup preset accounts
    let preset = gst_json.clone().pre;
    for (address, account_data) in preset {
        let without_prefix = account_data.balance.trim_start_matches("0x");
        let balance = u64::from_str_radix(without_prefix, 16).unwrap_or_default();
        // if an account is a wallet
        if account_data.code == *"0x" {
            runner.create_address(
                address_from_hex(&address),
                Wei::new_u64(balance),
                INITIAL_NONCE.into(),
            );
        }
        // or an externally owned account (e.g. precompile or contract)
        else {
            runner.create_address_with_code(
                address_from_hex(&address),
                Wei::new_u64(balance),
                INITIAL_NONCE.into(),
                hexstr_to_bytes(&account_data.code),
            );
        }
    }

    (runner, gst_json)
}

fn run(path: String, name: String) {
    let (mut runner, gst) = initialize_runner(path, name);

    // Bring up the test json file
    let gst_txs = gst.post("Merge".to_string());

    for i in gst_txs {
        let tx_post_bytes = &i.txbytes;
        let txbytes: Vec<u8> = hexstr_to_bytes(tx_post_bytes);
        let outcome: Result<(SubmitResult, ExecutionProfile), VMError> =
            runner.submit_transaction_raw(txbytes);
        assert!(outcome.is_ok());
        match outcome {
            Ok(result) => {
                println!("Result: {:?}", result.0);
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }
}

pub fn run_dir(dir: &str) {
    // get file names in the directory
    let paths = std::fs::read_dir(dir);
    if paths.is_err() {
        println!("Error: {:?}", paths.err().unwrap());
        return;
    }
    for path in paths.unwrap() {
        if path.is_err() {
            println!("Error: {:?}", path.err().unwrap());
            continue;
        }
        let path = path.unwrap().path();
        let path_str = path.to_str().unwrap().to_string();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        println!("Testing {} at {}", name.trim_end_matches(".json"), path_str);
        run(path_str, name.trim_end_matches(".json").to_string());
    }
}

// TODO: Spawn thread for each test
// //use std::thread;
// pub fn run_dir_batch(dir: &str) {
//     // get file names in the directory
//     let paths = std::fs::read_dir(dir).unwrap();
//     thread::spawn(|| {
//         for path in paths {
//             let path = path.unwrap().path();
//             let path_str = path.to_str().unwrap().to_string();
//             let name = path.file_name().unwrap().to_str().unwrap().to_string();
//             println!("Testing {} at {}", name.trim_end_matches(".json"), path_str);
//             run(path_str, name.trim_end_matches(".json").to_string());
//         }
//     });
// }

// Test individually
// cargo test --features mainnet-test  --package aurora-engine-tests --lib -- tests::ethereum_tests::general_state_tests --nocapture
#[test]
pub fn test_add() {
    run(
        "src/res/eth-tests/GeneralStateTests/VMTests/vmArithmeticTest/add.json".to_string(),
        "add".to_string(),
    )
}

#[test]
pub fn test_state_zero_one_balance() {
    let cwd = std::env::current_dir().unwrap();
    println!("Current directory is {}", cwd.display());
    run_dir("src/res/eth-tests/GeneralStateTests/stZeroOneBalance");
}

#[test]
pub fn test_state_attack_test() {
    let cwd = std::env::current_dir().unwrap();
    println!("Current directory is {}", cwd.display());

    run_dir("src/res/eth-tests/GeneralStateTests/stAttackTest");
}

#[test]
pub fn test_state_bad_opcode() {
    let cwd = std::env::current_dir().unwrap();
    println!("Current directory is {}", cwd.display());
    run_dir("src/res/eth-tests/GeneralStateTests/stBadOpcode");
}

#[test]
pub fn test_state_bugs() {
    let cwd = std::env::current_dir().unwrap();
    println!("Current directory is {}", cwd.display());
    run_dir("src/res/eth-tests/GeneralStateTests/stBugs");
}

#[test]
pub fn test_state_call_codes() {
    run_dir("src/res/eth-tests/GeneralStateTests/stCallCodes");
}

#[test]
pub fn test_state_call_create_call_code() {
    run_dir("src/res/eth-tests/GeneralStateTests/stCallCreateCallCodeTest");
}

#[test]
pub fn test_state_call_delegatecall_codecall() {
    run_dir("src/res/eth-tests/GeneralStateTests/stCallDelegateCodesCallCodeHomestead");
}

#[test]
pub fn test_state_call_delegatecall_homestead() {
    run_dir("src/res/eth-tests/GeneralStateTests/stCallDelegateCodesHomestead");
}

#[test]
pub fn test_state_chain_id() {
    run_dir("src/res/eth-tests/GeneralStateTests/stChainId");
}

#[test]
pub fn test_state_code_copy() {
    run_dir("src/res/eth-tests/GeneralStateTests/stCodeCopyTest");
}

#[test]
pub fn test_state_code_size_limit() {
    run_dir("src/res/eth-tests/GeneralStateTests/stCodeSizeLimit");
}

#[test]
pub fn test_state_create2() {
    run_dir("src/res/eth-tests/GeneralStateTests/stCreate2");
}

#[test]
pub fn test_state_create() {
    run_dir("src/res/eth-tests/GeneralStateTests/stCreateTest");
}

#[test]
pub fn test_state_delegatecall() {
    run_dir("src/res/eth-tests/GeneralStateTests/stDelegatecallTestHomestead");
}

#[test]
pub fn test_state_eip150_single_code_gas_prices() {
    run_dir("src/res/eth-tests/GeneralStateTests/stEIP150singleCodeGasPrices");
}

#[test]
pub fn test_state_eip150_specific() {
    run_dir("src/res/eth-tests/GeneralStateTests/stEIP150Specific");
}

#[test]
pub fn test_state_eip158_specific() {
    run_dir("src/res/eth-tests/GeneralStateTests/stEIP158");
}

#[test]
pub fn test_state_eip1559() {
    run_dir("src/res/eth-tests/GeneralStateTests/stEIP1559");
}

#[test]
pub fn test_state_eip2930() {
    run_dir("src/res/eth-tests/GeneralStateTests/stEIP2930");
}

#[test]
pub fn test_state_eip3607() {
    run_dir("src/res/eth-tests/GeneralStateTests/stEIP3607");
}

#[test]
pub fn test_state_example() {
    run_dir("src/res/eth-tests/GeneralStateTests/stExample");
}

#[test]
pub fn test_state_ext_code_hash() {
    run_dir("src/res/eth-tests/GeneralStateTests/stExtCodeHash");
}

#[test]
pub fn test_state_homestead_specific() {
    run_dir("src/res/eth-tests/GeneralStateTests/stHomesteadSpecific");
}

#[test]

pub fn test_state_init_code() {
    run_dir("src/res/eth-tests/GeneralStateTests/stInitCodeTest");
}

#[test]
pub fn test_state_log() {
    run_dir("src/res/eth-tests/GeneralStateTests/stLogTests");
}

#[test]
pub fn test_state_mem_expanding_eip150_calls() {
    run_dir("src/res/eth-tests/GeneralStateTests/stMemExpandingEIP150Calls");
}

#[test]
pub fn test_state_memory_stress() {
    run_dir("src/res/eth-tests/GeneralStateTests/stMemoryStressTest");
}

#[test]
pub fn test_state_memory() {
    run_dir("src/res/eth-tests/GeneralStateTests/stMemoryTest");
}

#[test]
pub fn test_state_non_zero_calls() {
    run_dir("src/res/eth-tests/GeneralStateTests/stNonZeroCallsTest");
}

#[test]
pub fn test_state_precompiles() {
    run_dir("src/res/eth-tests/GeneralStateTests/stPreCompiledContracts");
}

#[test]
pub fn test_state_precompiles2() {
    run_dir("src/res/eth-tests/GeneralStateTests/stPreCompiledContracts2");
}

#[test]
pub fn test_state_quadratic_complexity() {
    run_dir("src/res/eth-tests/GeneralStateTests/stQuadraticComplexityTest");
}

#[test]
pub fn test_state_random() {
    run_dir("src/res/eth-tests/GeneralStateTests/stRandom");
}

#[test]
pub fn test_state_random2() {
    run_dir("src/res/eth-tests/GeneralStateTests/stRandom2");
}

#[test]
pub fn test_state_recursive_create() {
    run_dir("src/res/eth-tests/GeneralStateTests/stRecursiveCreate");
}

#[test]
pub fn test_state_refund() {
    run_dir("src/res/eth-tests/GeneralStateTests/stRefundTest");
}

#[test]
pub fn test_state_return_data() {
    run_dir("src/res/eth-tests/GeneralStateTests/stReturnDataTest");
}

#[test]
pub fn test_state_revert() {
    run_dir("src/res/eth-tests/GeneralStateTests/stRevertTest");
}

#[test]
pub fn test_state_self_balance() {
    run_dir("src/res/eth-tests/GeneralStateTests/stSelfBalance");
}

#[test]
pub fn test_state_shift() {
    run_dir("src/res/eth-tests/GeneralStateTests/stShift");
}

#[test]
pub fn test_state_sload() {
    run_dir("src/res/eth-tests/GeneralStateTests/stLoadTest");
}

#[test]
pub fn test_state_solidity() {
    run_dir("src/res/eth-tests/GeneralStateTests/stSolidityTest");
}

#[test]
pub fn test_state_special() {
    run_dir("src/res/eth-tests/GeneralStateTests/stSpecialTest");
}

#[test]
pub fn test_state_store() {
    run_dir("src/res/eth-tests/GeneralStateTests/stSStoreTests");
}

#[test]
pub fn test_state_stack() {
    run_dir("src/res/eth-tests/GeneralStateTests/stStackTests");
}

#[test]
pub fn test_state_static() {
    run_dir("src/res/eth-tests/GeneralStateTests/stStaticCall");
}

#[test]
pub fn test_static_flag_enabled() {
    run_dir("src/res/eth-tests/GeneralStateTests/stStaticFlagEnabled");
}

#[test]
pub fn test_state_system_operations() {
    run_dir("src/res/eth-tests/GeneralStateTests/stSystemOperationsTest");
}

#[test]
pub fn test_state_time_consuming() {
    run_dir("src/res/eth-tests/GeneralStateTests/stTimeConsuming");
}

#[test]
pub fn test_state_transaction() {
    run_dir("src/res/eth-tests/GeneralStateTests/stTransactionTest");
}

#[test]
pub fn test_state_wallet() {
    run_dir("src/res/eth-tests/GeneralStateTests/stWalletTest");
}

#[test]
pub fn test_state_zero_calls_revert() {
    run_dir("src/res/eth-tests/GeneralStateTests/stZeroCallsRevert");
}

#[test]
pub fn test_state_zero_calls() {
    run_dir("src/res/eth-tests/GeneralStateTests/stZeroCallsTest");
}

#[test]
pub fn test_state_zero_knowledge() {
    run_dir("src/res/eth-tests/GeneralStateTests/stZeroKnowledge");
}

#[test]
pub fn test_state_zero_knowledge2() {
    run_dir("src/res/eth-tests/GeneralStateTests/stZeroKnowledge2");
}

#[test]
pub fn test_state() {
    let cwd = std::env::current_dir().unwrap();
    println!("Current directory is {}", cwd.display());
    let paths = std::fs::read_dir("src/res/eth-tests/GeneralStateTests");
    if paths.is_err() {
        println!("Error: {:?}", paths.err().unwrap());
        return;
    }
    for path in paths.unwrap() {
        if path.is_err() {
            println!("Error: {:?}", path.err().unwrap());
            continue;
        }
        let path = path.unwrap().path();
        let path_str = path.to_str().unwrap().to_string();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        println!("Testing {} at {}", name, path_str);
        if name.starts_with("st") {
            run_dir(&path_str);
        }
    }
}

#[test]
pub fn test_vm_arithmetic() {
    run_dir("src/res/eth-tests/VMTests/vmArithmeticTest");
}

#[test]
pub fn test_vm_bitwise_logic_operation() {
    run_dir("src/res/eth-tests/VMTests/vmBitwiseLogicOperation");
}

#[test]
pub fn test_vm_io_and_flow_operations() {
    run_dir("src/res/eth-tests/VMTests/vmIOandFlowOperations");
}

#[test]
pub fn test_vm_log() {
    run_dir("src/res/eth-tests/VMTests/vmLogTest");
}

#[test]
pub fn test_vm_performance() {
    run_dir("src/res/eth-tests/VMTests/vmPerformance");
}

#[test]
pub fn test_vm_tests() {
    run_dir("src/res/eth-tests/VMTests/vmTests");
}

#[test]
pub fn test_vm() {
    let paths = std::fs::read_dir("src/res/eth-tests/GeneralStateTests/VMTests");
    if paths.is_err() {
        println!("Error: {:?}", paths.err().unwrap());
        return;
    }
    for path in paths.unwrap() {
        if path.is_err() {
            println!("Error: {:?}", path.err().unwrap());
            continue;
        }
        let path = path.unwrap().path();
        let path_str = path.to_str().unwrap().to_string();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        println!("Testing {} at {}", name, path_str);
        run_dir(&path_str);
    }
}

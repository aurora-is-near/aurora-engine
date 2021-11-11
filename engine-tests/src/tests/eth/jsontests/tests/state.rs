use evm_jsontests::state as statetests;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;

pub fn run(dir: &str) {
	let _ = env_logger::try_init();

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
		let coll = serde_json::from_reader::<_, HashMap<String, statetests::Test>>(reader)
			.expect("Parse test cases failed");

		for (name, test) in coll {
			statetests::test(&name, test);
		}
	}
}

#[test]
fn st_args_zero_one_balance() {
	run("res/ethtests/GeneralStateTests/stArgsZeroOneBalance")
}
#[test]
fn st_attack() {
	run("res/ethtests/GeneralStateTests/stAttackTest")
}
#[test]
fn st_bad_opcode() {
	run("res/ethtests/GeneralStateTests/stBadOpcode")
}
#[test]
fn st_bugs() {
	run("res/ethtests/GeneralStateTests/stBugs")
}
#[test]
fn st_call_code() {
	run("res/ethtests/GeneralStateTests/stCallCodes")
}
#[test]
fn st_call_create_call_code() {
	run("res/ethtests/GeneralStateTests/stCallCreateCallCodeTest")
}
#[test]
fn st_call_delegate_codes_call_code_homestead() {
	run("res/ethtests/GeneralStateTests/stCallDelegateCodesCallCodeHomestead")
}
#[test]
fn st_call_delegate_codes_homestead() {
	run("res/ethtests/GeneralStateTests/stCallDelegateCodesHomestead")
}
#[test]
fn st_chain_id() {
	run("res/ethtests/GeneralStateTests/stChainId")
}
#[test]
fn st_code_copy() {
	run("res/ethtests/GeneralStateTests/stCodeCopyTest")
}
#[test]
fn st_code_size_limit() {
	run("res/ethtests/GeneralStateTests/stCodeSizeLimit")
}
#[test]
#[ignore]
fn st_create2() {
	run("res/ethtests/GeneralStateTests/stCreate2")
}
#[test]
fn st_create() {
	run("res/ethtests/GeneralStateTests/stCreateTest")
}
#[test]
fn st_delegate_call_homestead() {
	run("res/ethtests/GeneralStateTests/stDelegatecallTestHomestead")
}
#[test]
fn st_eip150_single_code_gas_prices() {
	run("res/ethtests/GeneralStateTests/stEIP150singleCodeGasPrices")
}
#[test]
fn st_eip150_specific() {
	run("res/ethtests/GeneralStateTests/stEIP150Specific")
}
#[test]
fn st_eip1559() {
	run("res/ethtests/GeneralStateTests/stEIP1559")
}
#[test]
fn st_eip158_specific() {
	run("res/ethtests/GeneralStateTests/stEIP158Specific")
}
#[test]
fn st_eip2930() {
	run("res/ethtests/GeneralStateTests/stEIP2930")
}
#[test]
fn st_example() {
	run("res/ethtests/GeneralStateTests/stExample")
}
#[test]
fn st_ext_code_hash() {
	run("res/ethtests/GeneralStateTests/stExtCodeHash")
}
#[test]
fn st_homestead_specific() {
	run("res/ethtests/GeneralStateTests/stHomesteadSpecific")
}
#[test]
fn st_init_code() {
	run("res/ethtests/GeneralStateTests/stInitCodeTest")
}
#[test]
fn st_log() {
	run("res/ethtests/GeneralStateTests/stLogTests")
}
#[test]
fn st_mem_expanding_eip_150_calls() {
	run("res/ethtests/GeneralStateTests/stMemExpandingEIP150Calls")
}
#[test]
fn st_memory_stress() {
	run("res/ethtests/GeneralStateTests/stMemoryStressTest")
}
#[test]
fn st_memory() {
	run("res/ethtests/GeneralStateTests/stMemoryTest")
}
#[test]
fn st_non_zero_calls() {
	run("res/ethtests/GeneralStateTests/stNonZeroCallsTest")
}
#[test]
fn st_precompiled_contracts() {
	run("res/ethtests/GeneralStateTests/stPreCompiledContracts")
}
#[test]
#[ignore]
fn st_precompiled_contracts2() {
	run("res/ethtests/GeneralStateTests/stPreCompiledContracts2")
}
#[test]
#[ignore]
fn st_quadratic_complexity() {
	run("res/ethtests/GeneralStateTests/stQuadraticComplexityTest")
}
#[test]
fn st_random() {
	run("res/ethtests/GeneralStateTests/stRandom")
}
#[test]
fn st_random2() {
	run("res/ethtests/GeneralStateTests/stRandom2")
}
#[test]
fn st_recursive_create() {
	run("res/ethtests/GeneralStateTests/stRecursiveCreate")
}
#[test]
fn st_refund() {
	run("res/ethtests/GeneralStateTests/stRefundTest")
}
#[test]
fn st_return_data() {
	run("res/ethtests/GeneralStateTests/stReturnDataTest")
}
#[test]
#[ignore]
fn st_revert() {
	run("res/ethtests/GeneralStateTests/stRevertTest")
}
#[test]
fn st_self_balance() {
	run("res/ethtests/GeneralStateTests/stSelfBalance")
}
#[test]
fn st_shift() {
	run("res/ethtests/GeneralStateTests/stShift")
}
#[test]
fn st_sload() {
	run("res/ethtests/GeneralStateTests/stSLoadTest")
}
#[test]
fn st_solidity() {
	run("res/ethtests/GeneralStateTests/stSolidityTest")
}
#[test]
#[ignore]
fn st_special() {
	run("res/ethtests/GeneralStateTests/stSpecialTest")
}
// Some of the collison test in sstore conflicts with evm's internal
// handlings. Those situations will never happen on a production chain (an empty
// account with storage values), so we can safely ignore them.
#[test]
#[ignore]
fn st_sstore() {
	run("res/ethtests/GeneralStateTests/stSStoreTest")
}
#[test]
fn st_stack() {
	run("res/ethtests/GeneralStateTests/stStackTests")
}
#[test]
#[ignore]
fn st_static_call() {
	run("res/ethtests/GeneralStateTests/stStaticCall")
}
#[test]
fn st_system_operations() {
	run("res/ethtests/GeneralStateTests/stSystemOperationsTest")
}
#[test]
fn st_transaction() {
	run("res/ethtests/GeneralStateTests/stTransactionTest")
}
#[test]
fn st_transition() {
	run("res/ethtests/GeneralStateTests/stTransitionTest")
}
#[test]
fn st_wallet() {
	run("res/ethtests/GeneralStateTests/stWalletTest")
}
#[test]
fn st_zero_calls_revert() {
	run("res/ethtests/GeneralStateTests/stZeroCallsRevert");
}
#[test]
fn st_zero_calls() {
	run("res/ethtests/GeneralStateTests/stZeroCallsTest")
}
#[test]
fn st_zero_knowledge() {
	run("res/ethtests/GeneralStateTests/stZeroKnowledge")
}
#[test]
fn st_zero_knowledge2() {
	run("res/ethtests/GeneralStateTests/stZeroKnowledge2")
}

use std::collections::BTreeMap;
use std::convert::TryInto;
use serde::Deserialize;
use primitive_types::{H160, H256, U256};
use evm::{Config, ExitSucceed, ExitError};
use evm::executor::{StackExecutor, MemoryStackState, StackSubstateMetadata, PrecompileOutput};
use evm::backend::{MemoryAccount, ApplyBackend, MemoryVicinity, MemoryBackend};
use parity_crypto::publickey;
use super::utils::*;

#[derive(Deserialize, Debug)]
pub struct Test(ethjson::test_helpers::state::State);

impl Test {
    pub fn unwrap_to_pre_state(&self) -> BTreeMap<H160, MemoryAccount> {
        unwrap_to_state(&self.0.pre_state)
    }

    pub fn unwrap_caller(&self) -> H160 {
        let secret_key: H256 = self.0.transaction.secret.clone().unwrap().into();
        let secret = publickey::Secret::import_key(&secret_key[..]).unwrap();
        let public = publickey::KeyPair::from_secret(secret).unwrap().public().clone();
        let sender = publickey::public_to_address(&public);

        sender
    }

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
/*
fn istanbul_precompile(
    address: H160,
    input: &[u8],
    target_gas: Option<u64>,
    _context: &evm::Context,
) -> Option<Result<PrecompileOutput, ExitError>> {
    use ethcore_builtin::*;
    use parity_bytes::BytesRef;

    let builtins: BTreeMap<ethjson::hash::Address, ethjson::spec::builtin::BuiltinCompat> = serde_json::from_str(include_str!("../res/istanbul_builtins.json")).unwrap();
    let builtins = builtins.into_iter().map(|(address, builtin)| {
        (address.into(), ethjson::spec::Builtin::from(builtin).try_into().unwrap())
    }).collect::<BTreeMap<H160, Builtin>>();

    if let Some(builtin) = builtins.get(&address) {
        let cost = builtin.cost(input, 0);

        if let Some(target_gas) = target_gas {
            if cost > U256::from(u64::max_value()) || target_gas < cost.as_u64() {
                return Some(Err(ExitError::OutOfGas))
            }
        }

        let mut output = Vec::new();
        match builtin.execute(input, &mut BytesRef::Flexible(&mut output)) {
            Ok(()) => Some(Ok(PrecompileOutput {
                exit_status: ExitSucceed::Stopped,
                output,
                cost: cost.as_u64(),
                logs: Vec::new(),
            })),
            Err(e) => Some(Err(ExitError::Other(e.into()))),
        }
    } else {
        None
    }
}
*/
pub fn test(name: &str, test: Test) {
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
    for (spec, states) in &test.0.post_states {
        let (gasometer_config, delete_empty) = match spec {
            ethjson::spec::ForkSpec::Istanbul => (Config::istanbul(), true),
            spec => {
                println!("Skip spec {:?}", spec);
                continue
            },
        };

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
        }
    }
}

mod engine_types;
use crate::engine_types::*;
mod async_promise;
use crate::async_promise::*;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{env, ext_contract, near_bindgen, assert_self, Promise, Gas};

pub const GAS_RESERVED_FOR_CURRENT_CALL: Gas = 20_000_000_000_000;

#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Eq, Clone)]
pub struct AsyncAuroraSubmitArgs {
    input: Vec<u8>,
    silo_account_id: String,
    submit_gas: Gas,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Default)]
pub struct AsyncAurora {
}

#[ext_contract(ext_aurora)]
pub trait Aurora {
    #[result_serializer(borsh)]
    fn submit(
        &mut self,
        #[serializer(borsh)]
        input : Vec<u8>,
    ) -> Promise<SubmitResult>;
}

#[ext_contract(ext_self)]
pub trait ExtAsyncAurora {
    #[result_serializer(borsh)]
    fn call_back(
        &mut self,
        #[callback]
        #[serializer(borsh)]
        result: SubmitResult,
    ) -> Promise;
}

#[near_bindgen]
impl AsyncAurora {
    pub fn submit(
        &self,
        #[serializer(borsh)] args: AsyncAuroraSubmitArgs,
    ) -> Promise { 
        Promise::new(args.silo_account_id).function_call(
            b"submit".to_vec(),
            args.input,
            0,
            args.submit_gas,
        ).then(ext_self::call_back(
            &env::current_account_id(), 
            env::attached_deposit(), 
            env::prepaid_gas() - args.submit_gas - GAS_RESERVED_FOR_CURRENT_CALL
        ))
    }

    pub fn call_back(
        &self,
        #[callback]
        #[serializer(borsh)]
        result: SubmitResult) {
        assert_self();    
        let output: Vec<u8> = match result.status {
            TransactionStatus::Succeed(ret) => ret,
            other => panic!("Unexpected status: {:?}", other),
        };

        let promises_str = ethabi::decode(&[ethabi::ParamType::String], output.as_slice()).unwrap().pop().unwrap().to_string().unwrap();
        let promises_desc = parse_promises(promises_str);
        let mut promises: Vec<Promise> = Vec::with_capacity(promises_desc.len());
        for promise_desc in promises_desc {
            let mut promise = Promise::new(promise_desc.target.clone()).function_call(
                promise_desc.method_name.clone(),
                promise_desc.arguments.clone(),
                0,
                promise_desc.gas,
            ).as_return();

            if let Some(combinator) = &promise_desc.combinator {
                match combinator.combinator_type {
                    CombinatorType::And => {
                        promise = promises[combinator.promise_index as usize].clone().and(promise);
                    },
                    CombinatorType::Then => {
                        promise = promises[combinator.promise_index as usize].clone().then(promise);
                    },
                }
            }

            promises.push(promise);
        }
    }
}

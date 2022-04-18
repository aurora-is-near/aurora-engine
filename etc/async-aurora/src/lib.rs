mod engine_types;
use crate::engine_types::*;
mod async_promise;
use crate::async_promise::*;
use near_sdk::assert_self;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{env, ext_contract, near_bindgen, AccountId, PanicOnDefault, Promise};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct AsyncAurora {
}

#[ext_contract(ext_aurora)]
pub trait Aurora {
    fn submit(
        &mut self,
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
    pub fn call(
        &self,
        input: Vec<u8>,
        silo_account_id: AccountId,
    ) -> Promise {
        ext_aurora::submit(
            input,
            &silo_account_id,
            0, 
            env::prepaid_gas()
        ).then(ext_self::call_back(
            &env::current_account_id(), 
            env::attached_deposit(), 
            env::prepaid_gas()
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
            _other => panic!("Submit transaction failed"),
        };

        let promises_desc = parse_promises(std::str::from_utf8(output.as_slice()).expect(ERR_INVALID_PROMISE).to_string());
        let mut promises: Vec<Promise> = Vec::new();
        for (ix, promise_desc) in promises_desc.iter().enumerate() {
            let promise = Promise::new(promise_desc.target.clone()).function_call(
                promise_desc.method_name.clone(),
                promise_desc.arguments.clone(),
                0,
                promise_desc.gas,
            );

            promises.push(promise.clone());

            match &promise_desc.combinator {
                Some(combinator) => match combinator.combinator_type {
                    CombinatorType::And => {
                        promises[ix] = promises[combinator.promise_index as usize].clone().and(promise);
                    },
                    CombinatorType::Then => {
                        promises[ix] = promises[combinator.promise_index as usize].clone().then(promise);
                    },
                },
                None => promises[ix] = promise,
            }
        }
    }
}

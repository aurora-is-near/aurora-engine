mod engine_types;
use crate::engine_types::*;
use near_sdk::assert_self;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{env, ext_contract, near_bindgen, AccountId, PanicOnDefault, Promise};


near_sdk::setup_alloc!();

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct AsyncAurora {
}

#[ext_contract(ext_bridge_token_factory)]
pub trait ExtBridgeTokenFactory {
    #[result_serializer(borsh)]
    fn finish_withdraw(
        &self,
        #[serializer(borsh)] amount: Balance,
        #[serializer(borsh)] recipient: AccountId,
    ) -> Promise;
}

#[ext_contract(ext_aurora)]
pub trait Aurora {
    #[result_serializer(borsh)]
    fn submit(
        &mut self,
        #[serializer(borsh)] payload : FunctionCallArgs,
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

const ERR_INVALID_PROMISE: &str = "Invalid promise format";

enum CombinatorType {
    And,
    Then,
}

struct CombinatorDescription {
    promise_index: u8,
    combinator_type: CombinatorType,
}

struct PromiseDescription {
    target: AccountId,
    method_name: Vec<u8>,
    arguments: Vec<u8>,
    gas: u64,
    combinator: Option<CombinatorDescription>,
}

fn parse_promises(input: String) -> Vec<PromiseDescription> {
    let striped_input = input.strip_prefix("promises:").expect(ERR_INVALID_PROMISE);
    let iter_list = striped_input.split("##");
    let mut result = Vec::new();

    for item in iter_list {
        let mut iter = item.split('#');
        let target = iter.next().expect(ERR_INVALID_PROMISE).into();
        let method_name = iter.next().expect(ERR_INVALID_PROMISE).into();
        let arguments = iter.next().expect(ERR_INVALID_PROMISE).into();
        let gas = iter.next().expect(ERR_INVALID_PROMISE).parse::<u64>().expect(ERR_INVALID_PROMISE);
        let mut combinator: Option<CombinatorDescription> = None;

        if let Some(index_str) = iter.next() {
            let promise_index = index_str.parse::<u8>().expect(ERR_INVALID_PROMISE);
            let type_str = iter.next().expect(ERR_INVALID_PROMISE);
            let combinator_type = if type_str == "&" {CombinatorType::And} else {CombinatorType::Then};
            combinator = Some(CombinatorDescription {
                promise_index,
                combinator_type,
            });
        }

        let promise_description = PromiseDescription {
            target,
            method_name,
            arguments,
            gas,
            combinator: combinator,
        };
        result.push(promise_description);
    }

    result
}

#[near_bindgen]
impl AsyncAurora {
    pub fn call(
        &self,
        #[serializer(borsh)]
        args: FunctionCallArgs,
        #[serializer(borsh)]
        silo_account_id: AccountId,
    ) -> Promise {
        assert_eq!(args.value, [0; 32], "Value should be 0");
        ext_aurora::submit(
            args,
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

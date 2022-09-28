use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::U128;
use near_sdk::{env, near_bindgen, Gas, Promise, PromiseError};

const FIVE_TGAS: Gas = Gas(5_000_000_000_000);

#[near_bindgen]
#[derive(Default, BorshDeserialize, BorshSerialize)]
pub struct Fib;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct FibAcc {
    a: U128,
    b: U128,
}

#[near_bindgen]
impl Fib {
    pub fn seed() -> FibAcc {
        FibAcc {
            a: U128(0),
            b: U128(1),
        }
    }

    #[handle_result]
    pub fn accumulate(
        #[callback_result] acc: Result<FibAcc, PromiseError>,
    ) -> Result<FibAcc, &'static str> {
        match acc {
            Ok(acc) => Ok(FibAcc {
                a: acc.b,
                b: U128(acc.a.0 + acc.b.0),
            }),
            Err(_) => Err("Promise failed"),
        }
    }

    pub fn fib(n: u8) -> Promise {
        let account = env::current_account_id();
        let mut p =
            Promise::new(account.clone()).function_call("seed".into(), Vec::new(), 0, FIVE_TGAS);
        let mut n = n;
        while n > 0 {
            n -= 1;
            p = p.then(Promise::new(account.clone()).function_call(
                "accumulate".into(),
                Vec::new(),
                0,
                FIVE_TGAS,
            ))
        }
        p
    }
}

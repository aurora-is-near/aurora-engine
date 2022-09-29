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
    /// Seeds the Fibonacci recursion with the starting values (i.e. F[0] and F[1]).
    pub fn seed() -> FibAcc {
        FibAcc {
            a: U128(0),
            b: U128(1),
        }
    }

    /// Performs one step of the Fibonacci recursion.
    #[handle_result]
    pub fn accumulate(
        #[callback_result] acc: Result<FibAcc, PromiseError>,
    ) -> Result<FibAcc, &'static str> {
        match acc {
            Ok(acc) => Ok(FibAcc {
                a: acc.b,
                b: u128_sum(acc.a, acc.b),
            }),
            Err(_) => Err("Promise failed"),
        }
    }

    /// Computes the nth Fibonacci number using NEAR cross-contract calls to this contract.
    /// It begins with the seed, followed by `n` calls to the `accumulate` function.
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

    /// An alternative version of `accumulate`. Rather then performing the recursion
    /// with a single input which contains the previous two Fibonacci values, this function
    /// takes the previous two terms as separate inputs. This gives an alternate way to compute
    /// Fibonacci numbers using this contract: `fib(n - 1).and(fib(n - 2)).then(sum)`.
    #[handle_result]
    pub fn sum(
        #[callback_result] fib_n_minus_1: Result<FibAcc, PromiseError>,
        #[callback_result] fib_n_minus_2: Result<FibAcc, PromiseError>,
    ) -> Result<FibAcc, String> {
        let FibAcc {
            a: fib_n_minus_1,
            b: fib_n,
        } = fib_n_minus_1.map_err(|e| format!("Promise 1 failed {:?}", e))?;
        let FibAcc {
            a: fib_n_minus_2,
            b: _,
        } = fib_n_minus_2.map_err(|e| format!("Promise 2 failed {:?}", e))?;
        Ok(FibAcc {
            a: u128_sum(fib_n_minus_1, fib_n_minus_2),
            b: u128_sum(fib_n_minus_1, fib_n),
        })
    }
}

fn u128_sum(x: U128, y: U128) -> U128 {
    U128(x.0 + y.0)
}

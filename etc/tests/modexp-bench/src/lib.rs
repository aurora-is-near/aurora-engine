use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{env, near_bindgen};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Default)]
pub struct Modexp;

#[near_bindgen]
impl Modexp {
    pub fn modexp(
        &self,
        base: String,
        exp: String,
        modulus: String,
        n_iters: Option<usize>,
    ) -> String {
        bench_modexp(base, exp, modulus, aurora_engine_modexp::modexp, n_iters)
    }

    pub fn modexp_ibig(
        &self,
        base: String,
        exp: String,
        modulus: String,
        n_iters: Option<usize>,
    ) -> String {
        bench_modexp(
            base,
            exp,
            modulus,
            aurora_engine_modexp::modexp_ibig,
            n_iters,
        )
    }

    pub fn modexp_num(
        &self,
        base: String,
        exp: String,
        modulus: String,
        n_iters: Option<usize>,
    ) -> String {
        bench_modexp(
            base,
            exp,
            modulus,
            aurora_engine_modexp::modexp_num,
            n_iters,
        )
    }
}

fn bench_modexp(
    base: String,
    exp: String,
    modulus: String,
    function: fn(&[u8], &[u8], &[u8]) -> Vec<u8>,
    n_iters: Option<usize>,
) -> String {
    let (base_bytes, exp_bytes, mod_bytes) = parse_input(base, exp, modulus);
    let output = function(&base_bytes, &exp_bytes, &mod_bytes);
    if let Some(n_iters) = n_iters {
        for _ in 1..n_iters {
            function(&base_bytes, &exp_bytes, &mod_bytes);
        }
    }
    hex::encode(output)
}

fn parse_input(base: String, exp: String, modulus: String) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let base_bytes = hex::decode(base).unwrap_or_else(hex_failure);
    let exp_bytes = hex::decode(exp).unwrap_or_else(hex_failure);
    let mod_bytes = hex::decode(modulus).unwrap_or_else(hex_failure);
    (base_bytes, exp_bytes, mod_bytes)
}

fn hex_failure<E, T>(_e: E) -> T {
    env::panic_str("Invalid hex input");
}

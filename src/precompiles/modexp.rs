use crate::precompiles::{Berlin, Byzantium, HardFork, Precompile, PrecompileResult};
use crate::prelude::{PhantomData, Vec, U256};
use evm::{Context, ExitError, ExitSucceed};
use num::BigUint;

pub(super) struct ModExp<HF: HardFork>(PhantomData<HF>);

impl ModExp<Byzantium> {
    fn adj_exp_len(exp_len: U256, base_len: U256, bytes: &[u8]) -> U256 {
        let mut exp32_bytes = Vec::with_capacity(32);
        for i in 0..32 {
            if U256::from(96) + base_len + U256::from(1) >= U256::from(bytes.len()) {
                exp32_bytes.push(0u8);
            } else {
                let base_len_i = base_len.as_usize();
                let bytes_i = 96 + base_len_i + i;
                if let Some(byte) = bytes.get(bytes_i) {
                    exp32_bytes.push(*byte);
                } else {
                    // Pad out the data if the byte is empty.
                    exp32_bytes.push(0u8);
                }
            }
        }
        let exp32 = U256::from(exp32_bytes.as_slice());

        if exp_len <= U256::from(32) && exp32 == U256::zero() {
            U256::zero()
        } else if exp_len <= U256::from(32) {
            U256::from(exp32.bits())
        } else {
            // else > 32
            U256::from(8) * (exp_len - U256::from(32)) + U256::from(exp32.bits())
        }
    }

    fn mult_complexity(x: U256) -> Result<U256, ExitError> {
        if x <= U256::from(64) {
            Ok(x * x)
        } else if x <= U256::from(1_024) {
            Ok(x * x / U256::from(4) + U256::from(96) * x - U256::from(3_072))
        } else {
            let (sqroot, overflow) = x.overflowing_mul(x);
            if overflow {
                Err(ExitError::OutOfGas)
            } else {
                Ok(sqroot / U256::from(16) + U256::from(480) * x - U256::from(199_680))
            }
        }
    }
}

impl Precompile for ModExp<Byzantium> {
    fn required_gas(input: &[u8]) -> Result<u64, ExitError> {
        let base_len = U256::from(&input[0..32]);
        let exp_len = U256::from(&input[32..64]);
        let mod_len = U256::from(&input[64..96]);

        let mul = Self::mult_complexity(core::cmp::max(mod_len, base_len))?;
        let adj = core::cmp::max(Self::adj_exp_len(exp_len, base_len, &input), U256::from(1))
            / U256::from(20);
        let (gas_val, overflow) = mul.overflowing_mul(adj);
        if overflow {
            Err(ExitError::OutOfGas)
        } else {
            Ok(gas_val.as_u64())
        }
    }

    /// See: https://eips.ethereum.org/EIPS/eip-198
    /// See: https://etherscan.io/address/0000000000000000000000000000000000000005
    fn run(input: &[u8], target_gas: u64, _context: &Context) -> PrecompileResult {
        if Self::required_gas(input)? > target_gas {
            return Err(ExitError::OutOfGas);
        }

        let base_len = U256::from(&input[0..32]);
        let exp_len = U256::from(&input[32..64]);
        let mod_len = U256::from(&input[64..96]);

        let base_len = base_len.as_usize();
        let mut base_bytes = Vec::with_capacity(32);
        for i in 0..base_len {
            if 96 + i >= input.len() {
                base_bytes.push(0u8);
            } else {
                base_bytes.push(input[96 + i]);
            }
        }

        let exp_len = exp_len.as_usize();
        let mut exp_bytes = Vec::with_capacity(32);
        for i in 0..exp_len {
            if 96 + base_len + i >= input.len() {
                exp_bytes.push(0u8);
            } else {
                exp_bytes.push(input[96 + base_len + i]);
            }
        }

        let mod_len = mod_len.as_usize();
        let mut mod_bytes = Vec::with_capacity(32);
        for i in 0..mod_len {
            if 96 + base_len + exp_len + i >= input.len() {
                mod_bytes.push(0u8);
            } else {
                mod_bytes.push(input[96 + base_len + exp_len + i]);
            }
        }

        let base = BigUint::from_bytes_be(&base_bytes);
        let exponent = BigUint::from_bytes_be(&exp_bytes);
        let modulus = BigUint::from_bytes_be(&mod_bytes);

        Ok((
            ExitSucceed::Returned,
            base.modpow(&exponent, &modulus).to_bytes_be(),
            0,
        ))
    }
}

impl Precompile for ModExp<Berlin> {
    fn required_gas(_input: &[u8]) -> Result<u64, ExitError> {
        todo!()
    }

    fn run(_input: &[u8], _target_gas: u64, _context: &Context) -> PrecompileResult {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_context() -> Context {
        Context {
            address: Default::default(),
            caller: Default::default(),
            apparent_value: Default::default(),
        }
    }

    #[test]
    fn test_modexp() {
        let test_input1 = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000001\
            0000000000000000000000000000000000000000000000000000000000000020\
            0000000000000000000000000000000000000000000000000000000000000020\
            03\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f",
        )
        .unwrap();
        let modexp_res = ModExp::<Byzantium>::run(&test_input1, 12_288, &new_context())
            .unwrap()
            .1;
        let res = U256::from_big_endian(&modexp_res);

        assert_eq!(res, U256::from(1));

        let test_input2 = hex::decode(
            "0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000020\
            0000000000000000000000000000000000000000000000000000000000000020\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f",
        )
        .unwrap();
        let modexp_res = ModExp::<Byzantium>::run(&test_input2, 12_288, &new_context())
            .unwrap()
            .1;
        let res = U256::from_big_endian(&modexp_res);

        assert_eq!(res, U256::from(0));

        let test_input3 = hex::decode(
            "0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000020\
            ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd",
        )
        .unwrap();
        assert!(ModExp::<Byzantium>::run(&test_input3, 0, &new_context()).is_err());

        let test_input4 = hex::decode(
            "0000000000000000000000000000000000000000000000000000000000000001\
            0000000000000000000000000000000000000000000000000000000000000002\
            0000000000000000000000000000000000000000000000000000000000000020\
            03\
            ffff\
            8000000000000000000000000000000000000000000000000000000000000000\
            07",
        )
        .unwrap();
        let expected = U256::from_big_endian(
            &hex::decode("3b01b01ac41f2d6e917c6d6a221ce793802469026d9ab7578fa2e79e4da6aaab")
                .unwrap(),
        );
        let modexp_res = ModExp::<Byzantium>::run(&test_input4, 12_288, &new_context())
            .unwrap()
            .1;
        let res = U256::from_big_endian(&modexp_res);
        assert_eq!(res, expected);

        let test_input5 = hex::decode(
            "0000000000000000000000000000000000000000000000000000000000000001\
            0000000000000000000000000000000000000000000000000000000000000002\
            0000000000000000000000000000000000000000000000000000000000000020\
            03\
            ffff\
            80",
        )
        .unwrap();
        let expected = U256::from_big_endian(
            &hex::decode("3b01b01ac41f2d6e917c6d6a221ce793802469026d9ab7578fa2e79e4da6aaab")
                .unwrap(),
        );
        let modexp_res = ModExp::<Byzantium>::run(&test_input5, 12_288, &new_context())
            .unwrap()
            .1;
        let res = U256::from_big_endian(&modexp_res);
        assert_eq!(res, expected);
    }
}

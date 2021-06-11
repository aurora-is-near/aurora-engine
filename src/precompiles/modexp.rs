use crate::precompiles::{
    Berlin, Byzantium, HardFork, Precompile, PrecompileOutput, PrecompileResult,
};
use crate::prelude::{PhantomData, Vec, U256};
use evm::{Context, ExitError};
use num::BigUint;

pub(super) const ADDRESS: [u8; 20] = super::make_address(0, 5);

pub(super) struct ModExp<HF: HardFork>(PhantomData<HF>);

impl<HF: HardFork> ModExp<HF> {
    fn adj_exp_len(exp_len: U256, base_len: U256, bytes: &[u8]) -> U256 {
        let mut exp_bytes = Vec::with_capacity(32);
        for i in 0..exp_len.as_usize() {
            if U256::from(96) + base_len + U256::from(1) >= U256::from(bytes.len()) {
                exp_bytes.push(0u8);
            } else {
                let bytes_i = 96 + base_len.as_usize() + i;
                if let Some(byte) = bytes.get(bytes_i) {
                    exp_bytes.push(*byte);
                } else {
                    // Pad out the data if the byte is empty.
                    exp_bytes.push(0u8);
                }
            }
        }

        let exp = U256::from(exp_bytes.as_slice());

        if exp_len <= U256::from(32) && exp == U256::zero() {
            U256::zero()
        } else if exp_len <= U256::from(32) {
            U256::from(exp.bits())
        } else {
            // else > 32
            U256::from(8) * (exp_len - U256::from(32)) + U256::from(exp.bits())
        }
    }

    fn run_inner(input: &[u8]) -> Result<Vec<u8>, ExitError> {
        let base_len = U256::from(&input[0..32]).as_usize();
        let exp_len = U256::from(&input[32..64]).as_usize();
        let mod_len = U256::from(&input[64..96]).as_usize();

        let mut base_bytes = Vec::with_capacity(32);
        for i in 0..base_len {
            if 96 + i >= input.len() {
                base_bytes.push(0u8);
            } else {
                base_bytes.push(input[96 + i]);
            }
        }

        let mut exp_bytes = Vec::with_capacity(32);
        for i in 0..exp_len {
            if 96 + base_len + i >= input.len() {
                exp_bytes.push(0u8);
            } else {
                exp_bytes.push(input[96 + base_len + i]);
            }
        }

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

        let output = {
            let computed_result = base.modpow(&exponent, &modulus).to_bytes_be();
            // The result must be the same length as the input modulus.
            // To ensure this we pad on the left with zeros.
            if mod_len > computed_result.len() {
                let diff = mod_len - computed_result.len();
                let mut padded_result = Vec::with_capacity(mod_len);
                padded_result.extend(core::iter::repeat(0).take(diff));
                padded_result.extend_from_slice(&computed_result);
                padded_result
            } else {
                computed_result
            }
        };

        Ok(output)
    }
}

impl ModExp<Byzantium> {
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
        let adj = Self::adj_exp_len(exp_len, base_len, &input) - U256::from(1);
        let (mut gas, overflow) =  mul.overflowing_mul(core::cmp::max(adj, U256::from(1)));
        if overflow {
            Err(ExitError::OutOfGas)
        } else {
            gas /= U256::from(20);
            Ok(gas.as_u64())
        }

        // let mul = Self::mult_complexity(core::cmp::max(mod_len, base_len))?;
        // let adj = core::cmp::max(Self::adj_exp_len(exp_len, base_len, &input));
        // let (gas_val, overflow) = mul.overflowing_mul(adj);
        // if overflow {
        //     Err(ExitError::OutOfGas)
        // } else {
        //     Ok(gas_val.as_u64())
        // }
    }

    /// See: https://eips.ethereum.org/EIPS/eip-198
    /// See: https://etherscan.io/address/0000000000000000000000000000000000000005
    fn run(input: &[u8], target_gas: u64, _context: &Context) -> PrecompileResult {
        let cost = Self::required_gas(input)?;
        if cost > target_gas {
            Err(ExitError::OutOfGas)
        } else {
            let output = Self::run_inner(input)?;
            Ok(PrecompileOutput::without_logs(cost, output))
        }
    }
}

impl ModExp<Berlin> {
    fn mult_complexity(mut x: U256) {
        x += U256::from(7);
        x /= U256::from(8);
        x *= x;
    }
}

impl Precompile for ModExp<Berlin> {
    fn required_gas(input: &[u8]) -> Result<u64, ExitError> {
        let base_len = U256::from(&input[0..32]);
        let exp_len = U256::from(&input[32..64]);
        let mod_len = U256::from(&input[64..96]);

        let adj = Self::adj_exp_len(exp_len, base_len, &input);

        // Three changes in EIP-2565
        //
        // 1. Different mult complexity in EIP-2565
        // (https://eips.ethereum.org/EIPS/eip-2565).
        //
        // def mult_complexity(x):
        //    ceiling(x/8)^2
        //
        // where x is max(length_of_MODULUS, length_of_BASE)
        let mut gas = core::cmp::max(mod_len, base_len);
        Self::mult_complexity(gas);

        gas *= core::cmp::max(adj, U256::from(1));

        // 2. Different divisor (`GQUADDIVISOR`) (3)
        gas /= U256::from(3);
        if gas.bits() > 64 {
            return Ok(u64::MAX);
        }

        // 3. Minimum price of 200 gas
        if gas.as_u64() < 200 {
            return Ok(200u64);
        }

        Ok(gas.as_u64())
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

    struct Test {
        input: &'static str,
        expected: &'static str,
        name: &'static str,
    }

    const BYZANTIUM_TESTS: [Test; 3] = [
        Test {
            input: "\
            0000000000000000000000000000000000000000000000000000000000000001\
            0000000000000000000000000000000000000000000000000000000000000020\
            0000000000000000000000000000000000000000000000000000000000000020\
            03\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f",
            expected: "0000000000000000000000000000000000000000000000000000000000000001",
            name: "eip198_example_1",
        },
        Test {
            input: "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000020\
            0000000000000000000000000000000000000000000000000000000000000020\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f",
            expected: "0000000000000000000000000000000000000000000000000000000000000000",
            name: "eip198_example_2",
        },
        Test {
            input: "\
            0000000000000000000000000000000000000000000000000000000000000040\
            0000000000000000000000000000000000000000000000000000000000000001\
            0000000000000000000000000000000000000000000000000000000000000040\
            e09ad9675465c53a109fac66a445c91b292d2bb2c5268addb30cd82f80fcb003\
            3ff97c80a5fc6f39193ae969c6ede6710a6b7ac27078a06d90ef1c72e5c85fb5\
            02fc9e1f6beb81516545975218075ec2af118cd8798df6e08a147c60fd6095ac\
            2bb02c2908cf4dd7c81f11c289e4bce98f3553768f392a80ce22bf5c4f4a248c\
            6b",
            expected: "60008f1614cc01dcfb6bfb09c625cf90b47d4468db81b5f8b7a39d42f332eab9b2da8f2d95311648a8f243f4bb13cfb3d8f7f2a3c014122ebb3ed41b02783adc",
            name: "nagydani_1_square",
        },
    ];

    const BYZANTIUM_GAS: [u64; 3] = [
        13_056,
        13_056,
        204,
    ];

    #[test]
    fn test_byzantium_modexp() {
        for (test, test_gas) in BYZANTIUM_TESTS.iter().zip(BYZANTIUM_GAS.iter()) {
            let input = hex::decode(&test.input).unwrap();

            let gas = ModExp::<Byzantium>::required_gas(&input).unwrap();
            assert_eq!(gas, *test_gas, "{} gas", test.name);

            let res = ModExp::<Byzantium>::run(&input, *test_gas, &new_context()).unwrap().output;
            let expected = hex::decode(&test.expected).unwrap();
            assert_eq!(res, expected, "{}", test.name);
        }

        // let test_input1 = hex::decode(
        //     "\
        //     0000000000000000000000000000000000000000000000000000000000000001\
        //     0000000000000000000000000000000000000000000000000000000000000020\
        //     0000000000000000000000000000000000000000000000000000000000000020\
        //     03\
        //     fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e\
        //     fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f",
        // )
        // .unwrap();
        // let modexp_res = ModExp::<Byzantium>::run(&test_input1, 12_288, &new_context())
        //     .unwrap()
        //     .output;
        // let res = U256::from_big_endian(&modexp_res);
        //
        // assert_eq!(res, U256::from(1));
        //
        // let test_input2 = hex::decode(
        //     "0000000000000000000000000000000000000000000000000000000000000000\
        //     0000000000000000000000000000000000000000000000000000000000000020\
        //     0000000000000000000000000000000000000000000000000000000000000020\
        //     fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e\
        //     fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f",
        // )
        // .unwrap();
        // let modexp_res = ModExp::<Byzantium>::run(&test_input2, 12_288, &new_context())
        //     .unwrap()
        //     .output;
        // let res = U256::from_big_endian(&modexp_res);
        //
        // assert_eq!(res, U256::from(0));
        //
        // let test_input3 = hex::decode(
        //     "0000000000000000000000000000000000000000000000000000000000000000\
        //     0000000000000000000000000000000000000000000000000000000000000020\
        //     ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff\
        //     fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe\
        //     fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd",
        // )
        // .unwrap();
        // assert!(ModExp::<Byzantium>::run(&test_input3, 0, &new_context()).is_err());
        //
        // let test_input4 = hex::decode(
        //     "0000000000000000000000000000000000000000000000000000000000000001\
        //     0000000000000000000000000000000000000000000000000000000000000002\
        //     0000000000000000000000000000000000000000000000000000000000000020\
        //     03\
        //     ffff\
        //     8000000000000000000000000000000000000000000000000000000000000000\
        //     07",
        // )
        // .unwrap();
        // let expected = U256::from_big_endian(
        //     &hex::decode("3b01b01ac41f2d6e917c6d6a221ce793802469026d9ab7578fa2e79e4da6aaab")
        //         .unwrap(),
        // );
        // let modexp_res = ModExp::<Byzantium>::run(&test_input4, 12_288, &new_context())
        //     .unwrap()
        //     .output;
        // let res = U256::from_big_endian(&modexp_res);
        // assert_eq!(res, expected);
        //
        // let test_input5 = hex::decode(
        //     "0000000000000000000000000000000000000000000000000000000000000001\
        //     0000000000000000000000000000000000000000000000000000000000000002\
        //     0000000000000000000000000000000000000000000000000000000000000020\
        //     03\
        //     ffff\
        //     80",
        // )
        // .unwrap();
        // let expected = U256::from_big_endian(
        //     &hex::decode("3b01b01ac41f2d6e917c6d6a221ce793802469026d9ab7578fa2e79e4da6aaab")
        //         .unwrap(),
        // );
        // let modexp_res = ModExp::<Byzantium>::run(&test_input5, 12_288, &new_context())
        //     .unwrap()
        //     .output;
        // let res = U256::from_big_endian(&modexp_res);
        // assert_eq!(res, expected);
    }

    #[test]
    fn test_berlin_modexp() {

    }
}

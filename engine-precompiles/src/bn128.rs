use crate::prelude::types::{Address, EthGas};
use crate::prelude::{Borrowed, PhantomData, Vec};
use crate::{Byzantium, EvmPrecompileResult, HardFork, Istanbul, Precompile, PrecompileOutput};
use evm::{Context, ExitError};

/// bn128 costs.
mod costs {
    use crate::prelude::types::EthGas;

    /// Cost of the Byzantium alt_bn128_add operation.
    pub(super) const BYZANTIUM_ADD: EthGas = EthGas::new(500);

    /// Cost of the Byzantium alt_bn128_mul operation.
    pub(super) const BYZANTIUM_MUL: EthGas = EthGas::new(40_000);

    /// Cost of the alt_bn128_pair per point.
    pub(super) const BYZANTIUM_PAIR_PER_POINT: EthGas = EthGas::new(80_000);

    /// Cost of the alt_bn128_pair operation.
    pub(super) const BYZANTIUM_PAIR_BASE: EthGas = EthGas::new(100_000);

    /// Cost of the Istanbul alt_bn128_add operation.
    pub(super) const ISTANBUL_ADD: EthGas = EthGas::new(150);

    /// Cost of the Istanbul alt_bn128_mul operation.
    pub(super) const ISTANBUL_MUL: EthGas = EthGas::new(6_000);

    /// Cost of the Istanbul alt_bn128_pair per point.
    pub(super) const ISTANBUL_PAIR_PER_POINT: EthGas = EthGas::new(34_000);

    /// Cost of the Istanbul alt_bn128_pair operation.
    pub(super) const ISTANBUL_PAIR_BASE: EthGas = EthGas::new(45_000);
}

/// bn128 constants.
mod consts {
    /// Input length for the add operation.
    pub(super) const ADD_INPUT_LEN: usize = 128;

    /// Input length for the multiplication operation.
    pub(super) const MUL_INPUT_LEN: usize = 128;

    /// Pair element length.
    pub(super) const PAIR_ELEMENT_LEN: usize = 192;
}

/// Reads the `x` and `y` points from an input at a given position.
fn read_point(input: &[u8], pos: usize) -> Result<bn::G1, ExitError> {
    use bn::{AffineG1, Fq, Group, G1};

    let mut px_buf = [0u8; 32];
    px_buf.copy_from_slice(&input[pos..(pos + 32)]);
    let px =
        Fq::interpret(&px_buf).map_err(|_e| ExitError::Other(Borrowed("ERR_BN128_INVALID_X")))?;

    let mut py_buf = [0u8; 32];
    py_buf.copy_from_slice(&input[(pos + 32)..(pos + 64)]);
    let py =
        Fq::interpret(&py_buf).map_err(|_e| ExitError::Other(Borrowed("ERR_BN128_INVALID_Y")))?;

    Ok(if px == Fq::zero() && py == bn::Fq::zero() {
        G1::zero()
    } else {
        AffineG1::new(px, py)
            .map_err(|_| ExitError::Other(Borrowed("ERR_BN128_INVALID_POINT")))?
            .into()
    })
}

pub(super) struct Bn128Add<HF: HardFork>(PhantomData<HF>);

impl<HF: HardFork> Bn128Add<HF> {
    pub(super) const ADDRESS: Address = super::make_address(0, 6);

    pub fn new() -> Self {
        Self(Default::default())
    }
}

impl<HF: HardFork> Bn128Add<HF> {
    fn run_inner(input: &[u8], _context: &Context) -> Result<Vec<u8>, ExitError> {
        use bn::AffineG1;

        let mut input = input.to_vec();
        input.resize(consts::ADD_INPUT_LEN, 0);

        let p1 = read_point(&input, 0)?;
        let p2 = read_point(&input, 64)?;

        let mut output = [0u8; 64];
        if let Some(sum) = AffineG1::from_jacobian(p1 + p2) {
            let x = sum.x().into_u256().to_big_endian();
            let y = sum.y().into_u256().to_big_endian();
            output[0..32].copy_from_slice(&x);
            output[32..64].copy_from_slice(&y);
        }

        Ok(output.to_vec())
    }
}

impl Precompile for Bn128Add<Byzantium> {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::BYZANTIUM_ADD)
    }

    /// Takes in two points on the elliptic curve alt_bn128 and calculates the sum
    /// of them.
    ///
    /// See: https://eips.ethereum.org/EIPS/eip-196
    /// See: https://etherscan.io/address/0000000000000000000000000000000000000006
    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        let cost = Self::required_gas(input)?;
        if let Some(target_gas) = target_gas {
            if cost > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        let output = Self::run_inner(input, context)?;
        Ok(PrecompileOutput::without_logs(cost, output).into())
    }
}

impl Precompile for Bn128Add<Istanbul> {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::ISTANBUL_ADD)
    }

    /// Takes in two points on the elliptic curve alt_bn128 and calculates the sum
    /// of them.
    ///
    /// See: https://eips.ethereum.org/EIPS/eip-196
    /// See: https://etherscan.io/address/0000000000000000000000000000000000000006
    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        let cost = Self::required_gas(input)?;
        if let Some(target_gas) = target_gas {
            if cost > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }
        let output = Self::run_inner(input, context)?;
        Ok(PrecompileOutput::without_logs(cost, output).into())
    }
}

pub(super) struct Bn128Mul<HF: HardFork>(PhantomData<HF>);

impl<HF: HardFork> Bn128Mul<HF> {
    pub(super) const ADDRESS: Address = super::make_address(0, 7);

    pub fn new() -> Self {
        Self(Default::default())
    }
}

impl<HF: HardFork> Bn128Mul<HF> {
    fn run_inner(input: &[u8], _context: &Context) -> Result<Vec<u8>, ExitError> {
        use bn::AffineG1;

        let mut input = input.to_vec();
        input.resize(consts::MUL_INPUT_LEN, 0);

        let p = read_point(&input, 0)?;
        let mut fr_buf = [0u8; 32];
        fr_buf.copy_from_slice(&input[64..96]);
        let fr = bn::Fr::interpret(&fr_buf)
            .map_err(|_e| ExitError::Other(Borrowed("ERR_BN128_INVALID_FE")))?;

        let mut output = [0u8; 64];
        if let Some(mul) = AffineG1::from_jacobian(p * fr) {
            let x = mul.x().into_u256().to_big_endian();
            let y = mul.y().into_u256().to_big_endian();
            output[0..32].copy_from_slice(&x);
            output[32..64].copy_from_slice(&y);
        }

        Ok(output.to_vec())
    }
}

impl Precompile for Bn128Mul<Byzantium> {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::BYZANTIUM_MUL)
    }

    /// Takes in two points on the elliptic curve alt_bn128 and multiples them.
    ///
    /// See: https://eips.ethereum.org/EIPS/eip-196
    /// See: https://etherscan.io/address/0000000000000000000000000000000000000007
    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        let cost = Self::required_gas(input)?;
        if let Some(target_gas) = target_gas {
            if cost > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        let output = Self::run_inner(input, context)?;
        Ok(PrecompileOutput::without_logs(cost, output).into())
    }
}

impl Precompile for Bn128Mul<Istanbul> {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::ISTANBUL_MUL)
    }

    /// Takes in two points on the elliptic curve alt_bn128 and multiples them.
    ///
    /// See: https://eips.ethereum.org/EIPS/eip-196
    /// See: https://etherscan.io/address/0000000000000000000000000000000000000007
    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        let cost = Self::required_gas(input)?;
        if let Some(target_gas) = target_gas {
            if cost > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        let output = Self::run_inner(input, context)?;
        Ok(PrecompileOutput::without_logs(cost, output).into())
    }
}

pub(super) struct Bn128Pair<HF: HardFork>(PhantomData<HF>);

impl<HF: HardFork> Bn128Pair<HF> {
    pub(super) const ADDRESS: Address = super::make_address(0, 8);

    pub fn new() -> Self {
        Self(Default::default())
    }
}

impl<HF: HardFork> Bn128Pair<HF> {
    fn run_inner(input: &[u8], _context: &Context) -> Result<Vec<u8>, ExitError> {
        use bn::{arith::U256, AffineG1, AffineG2, Fq, Fq2, Group, Gt, G1, G2};

        if input.len() % consts::PAIR_ELEMENT_LEN != 0 {
            return Err(ExitError::Other(Borrowed("ERR_BN128_INVALID_LEN")));
        }

        let output = if input.is_empty() {
            U256::one()
        } else {
            let elements = input.len() / consts::PAIR_ELEMENT_LEN;
            let mut vals = Vec::with_capacity(elements);

            for idx in 0..elements {
                let mut buf = [0u8; 32];

                buf.copy_from_slice(
                    &input[(idx * consts::PAIR_ELEMENT_LEN)..(idx * consts::PAIR_ELEMENT_LEN + 32)],
                );
                let ax = Fq::interpret(&buf)
                    .map_err(|_e| ExitError::Other(Borrowed("ERR_BN128_INVALID_AX")))?;
                buf.copy_from_slice(
                    &input[(idx * consts::PAIR_ELEMENT_LEN + 32)
                        ..(idx * consts::PAIR_ELEMENT_LEN + 64)],
                );
                let ay = Fq::interpret(&buf)
                    .map_err(|_e| ExitError::Other(Borrowed("ERR_BN128_INVALID_AY")))?;
                buf.copy_from_slice(
                    &input[(idx * consts::PAIR_ELEMENT_LEN + 64)
                        ..(idx * consts::PAIR_ELEMENT_LEN + 96)],
                );
                let bay = Fq::interpret(&buf)
                    .map_err(|_e| ExitError::Other(Borrowed("ERR_BN128_INVALID_B_AY")))?;
                buf.copy_from_slice(
                    &input[(idx * consts::PAIR_ELEMENT_LEN + 96)
                        ..(idx * consts::PAIR_ELEMENT_LEN + 128)],
                );
                let bax = Fq::interpret(&buf)
                    .map_err(|_e| ExitError::Other(Borrowed("ERR_BN128_INVALID_B_AX")))?;
                buf.copy_from_slice(
                    &input[(idx * consts::PAIR_ELEMENT_LEN + 128)
                        ..(idx * consts::PAIR_ELEMENT_LEN + 160)],
                );
                let bby = Fq::interpret(&buf)
                    .map_err(|_e| ExitError::Other(Borrowed("ERR_BN128_INVALID_B_BY")))?;
                buf.copy_from_slice(
                    &input[(idx * consts::PAIR_ELEMENT_LEN + 160)
                        ..(idx * consts::PAIR_ELEMENT_LEN + 192)],
                );
                let bbx = Fq::interpret(&buf)
                    .map_err(|_e| ExitError::Other(Borrowed("ERR_BN128_INVALID_B_BX")))?;

                let a = {
                    if ax.is_zero() && ay.is_zero() {
                        G1::zero()
                    } else {
                        G1::from(
                            AffineG1::new(ax, ay)
                                .map_err(|_e| ExitError::Other(Borrowed("ERR_BN128_INVALID_A")))?,
                        )
                    }
                };
                let b = {
                    let ba = Fq2::new(bax, bay);
                    let bb = Fq2::new(bbx, bby);

                    if ba.is_zero() && bb.is_zero() {
                        G2::zero()
                    } else {
                        G2::from(
                            AffineG2::new(ba, bb)
                                .map_err(|_e| ExitError::Other(Borrowed("ERR_BN128_INVALID_B")))?,
                        )
                    }
                };
                vals.push((a, b))
            }

            let mul = vals
                .into_iter()
                .fold(Gt::one(), |s, (a, b)| s * bn::pairing(a, b));

            if mul == Gt::one() {
                U256::one()
            } else {
                U256::zero()
            }
        };

        Ok(output.to_big_endian().to_vec())
    }
}

impl Precompile for Bn128Pair<Byzantium> {
    fn required_gas(input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(
            costs::BYZANTIUM_PAIR_PER_POINT * input.len() / consts::PAIR_ELEMENT_LEN
                + costs::BYZANTIUM_PAIR_BASE,
        )
    }

    /// Takes in elements and calculates the pair.
    ///
    /// See: https://eips.ethereum.org/EIPS/eip-197
    /// See: https://etherscan.io/address/0000000000000000000000000000000000000008
    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        let cost = Self::required_gas(input)?;
        if let Some(target_gas) = target_gas {
            if cost > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        let output = Self::run_inner(input, context)?;
        Ok(PrecompileOutput::without_logs(cost, output).into())
    }
}

impl Precompile for Bn128Pair<Istanbul> {
    fn required_gas(input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(
            costs::ISTANBUL_PAIR_PER_POINT * input.len() / consts::PAIR_ELEMENT_LEN
                + costs::ISTANBUL_PAIR_BASE,
        )
    }

    /// Takes in elements and calculates the pair.
    ///
    /// See: https://eips.ethereum.org/EIPS/eip-197
    /// See: https://etherscan.io/address/0000000000000000000000000000000000000008
    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        let cost = Self::required_gas(input)?;
        if let Some(target_gas) = target_gas {
            if cost > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        let output = Self::run_inner(input, context)?;
        Ok(PrecompileOutput::without_logs(cost, output).into())
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::new_context;

    use super::*;

    #[test]
    fn test_alt_bn128_add() {
        let input = hex::decode(
            "\
             18b18acfb4c2c30276db5411368e7185b311dd124691610c5d3b74034e093dc9\
             063c909c4720840cb5134cb9f59fa749755796819658d32efc0d288198f37266\
             07c2b7f58a84bd6145f00c9c2bc0bb1a187f20ff2c92963a88019e7c6a014eed\
             06614e20c147e940f2d70da3f74c9a17df361706a4485c742bd6788478fa17d7",
        )
        .unwrap();
        let expected = hex::decode(
            "\
            2243525c5efd4b9c3d3c45ac0ca3fe4dd85e830a4ce6b65fa1eeaee202839703\
            301d1d33be6da8e509df21cc35964723180eed7532537db9ae5e7d48f195c915",
        )
        .unwrap();

        let res = Bn128Add::<Byzantium>::new()
            .run(&input, Some(EthGas::new(500)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        // zero sum test
        let input = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();
        let expected = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();

        let res = Bn128Add::<Byzantium>::new()
            .run(&input, Some(EthGas::new(500)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        // out of gas test
        let input = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();
        let res =
            Bn128Add::<Byzantium>::new().run(&input, Some(EthGas::new(499)), &new_context(), false);
        assert!(matches!(res, Err(ExitError::OutOfGas)));

        // no input test
        let input = [0u8; 0];
        let expected = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();

        let res = Bn128Add::<Byzantium>::new()
            .run(&input, Some(EthGas::new(500)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        // point not on curve fail
        let input = hex::decode(
            "\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111",
        )
        .unwrap();

        let res =
            Bn128Add::<Byzantium>::new().run(&input, Some(EthGas::new(500)), &new_context(), false);
        assert!(matches!(
            res,
            Err(ExitError::Other(Borrowed("ERR_BN128_INVALID_POINT")))
        ));
    }

    #[test]
    fn test_alt_bn128_mul() {
        let input = hex::decode(
            "\
            2bd3e6d0f3b142924f5ca7b49ce5b9d54c4703d7ae5648e61d02268b1a0a9fb7\
            21611ce0a6af85915e2f1d70300909ce2e49dfad4a4619c8390cae66cefdb204\
            00000000000000000000000000000000000000000000000011138ce750fa15c2",
        )
        .unwrap();
        let expected = hex::decode(
            "\
            070a8d6a982153cae4be29d434e8faef8a47b274a053f5a4ee2a6c9c13c31e5c\
            031b8ce914eba3a9ffb989f9cdd5b0f01943074bf4f0f315690ec3cec6981afc",
        )
        .unwrap();

        let res = Bn128Mul::<Byzantium>::new()
            .run(&input, Some(EthGas::new(40_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        // out of gas test
        let input = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000\
            0200000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();
        let res = Bn128Mul::<Byzantium>::new().run(
            &input,
            Some(EthGas::new(39_999)),
            &new_context(),
            false,
        );
        assert!(matches!(res, Err(ExitError::OutOfGas)));

        // zero multiplication test
        let input = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000\
            0200000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();
        let expected = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();

        let res = Bn128Mul::<Byzantium>::new()
            .run(&input, Some(EthGas::new(40_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        // no input test
        let input = [0u8; 0];
        let expected = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();

        let res = Bn128Mul::<Byzantium>::new()
            .run(&input, Some(EthGas::new(40_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        // point not on curve fail
        let input = hex::decode(
            "\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            0f00000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();

        let res = Bn128Mul::<Byzantium>::new().run(
            &input,
            Some(EthGas::new(40_000)),
            &new_context(),
            false,
        );
        assert!(matches!(
            res,
            Err(ExitError::Other(Borrowed("ERR_BN128_INVALID_POINT")))
        ));
    }

    #[test]
    fn test_alt_bn128_pair() {
        let input = hex::decode(
            "\
            1c76476f4def4bb94541d57ebba1193381ffa7aa76ada664dd31c16024c43f59\
            3034dd2920f673e204fee2811c678745fc819b55d3e9d294e45c9b03a76aef41\
            209dd15ebff5d46c4bd888e51a93cf99a7329636c63514396b4a452003a35bf7\
            04bf11ca01483bfa8b34b43561848d28905960114c8ac04049af4b6315a41678\
            2bb8324af6cfc93537a2ad1a445cfd0ca2a71acd7ac41fadbf933c2a51be344d\
            120a2a4cf30c1bf9845f20c6fe39e07ea2cce61f0c9bb048165fe5e4de877550\
            111e129f1cf1097710d41c4ac70fcdfa5ba2023c6ff1cbeac322de49d1b6df7c\
            2032c61a830e3c17286de9462bf242fca2883585b93870a73853face6a6bf411\
            198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2\
            1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed\
            090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b\
            12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa",
        )
        .unwrap();
        let expected =
            hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();

        let res = Bn128Pair::<Byzantium>::new()
            .run(&input, Some(EthGas::new(260_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        // out of gas test
        let input = hex::decode(
            "\
            1c76476f4def4bb94541d57ebba1193381ffa7aa76ada664dd31c16024c43f59\
            3034dd2920f673e204fee2811c678745fc819b55d3e9d294e45c9b03a76aef41\
            209dd15ebff5d46c4bd888e51a93cf99a7329636c63514396b4a452003a35bf7\
            04bf11ca01483bfa8b34b43561848d28905960114c8ac04049af4b6315a41678\
            2bb8324af6cfc93537a2ad1a445cfd0ca2a71acd7ac41fadbf933c2a51be344d\
            120a2a4cf30c1bf9845f20c6fe39e07ea2cce61f0c9bb048165fe5e4de877550\
            111e129f1cf1097710d41c4ac70fcdfa5ba2023c6ff1cbeac322de49d1b6df7c\
            2032c61a830e3c17286de9462bf242fca2883585b93870a73853face6a6bf411\
            198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2\
            1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed\
            090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b\
            12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa",
        )
        .unwrap();
        let res = Bn128Pair::<Byzantium>::new().run(
            &input,
            Some(EthGas::new(259_999)),
            &new_context(),
            false,
        );
        assert!(matches!(res, Err(ExitError::OutOfGas)));

        // no input test
        let input = [0u8; 0];
        let expected =
            hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();

        let res = Bn128Pair::<Byzantium>::new()
            .run(&input, Some(EthGas::new(260_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        // point not on curve fail
        let input = hex::decode(
            "\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111",
        )
        .unwrap();

        let res = Bn128Pair::<Byzantium>::new().run(
            &input,
            Some(EthGas::new(260_000)),
            &new_context(),
            false,
        );
        assert!(matches!(
            res,
            Err(ExitError::Other(Borrowed("ERR_BN128_INVALID_A")))
        ));

        // invalid input length
        let input = hex::decode(
            "\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            111111111111111111111111111111\
        ",
        )
        .unwrap();

        let res = Bn128Pair::<Byzantium>::new().run(
            &input,
            Some(EthGas::new(260_000)),
            &new_context(),
            false,
        );
        assert!(matches!(
            res,
            Err(ExitError::Other(Borrowed("ERR_BN128_INVALID_LEN",)))
        ));
    }
}

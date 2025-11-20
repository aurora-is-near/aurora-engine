use crate::prelude::types::{make_address, Address, EthGas};
use crate::prelude::{Borrowed, PhantomData, Vec};
use crate::utils;
use crate::{Byzantium, EvmPrecompileResult, HardFork, Istanbul, Precompile, PrecompileOutput};
use aurora_engine_sdk::BnError;
use aurora_evm::{Context, ExitError};
use core::num::{NonZeroU64, NonZeroUsize};

/// bn128 costs.
mod costs {
    use crate::prelude::types::EthGas;

    /// Cost of the Byzantium `alt_bn128_add` operation.
    pub(super) const BYZANTIUM_ADD: EthGas = EthGas::new(500);

    /// Cost of the Byzantium `alt_bn128_mul` operation.
    pub(super) const BYZANTIUM_MUL: EthGas = EthGas::new(40_000);

    /// Cost of the `alt_bn128_pair` per point.
    pub(super) const BYZANTIUM_PAIR_PER_POINT: EthGas = EthGas::new(80_000);

    /// Cost of the `alt_bn128_pair` operation.
    pub(super) const BYZANTIUM_PAIR_BASE: EthGas = EthGas::new(100_000);

    /// Cost of the Istanbul `alt_bn128_add` operation.
    pub(super) const ISTANBUL_ADD: EthGas = EthGas::new(150);

    /// Cost of the Istanbul `alt_bn128_mul` operation.
    pub(super) const ISTANBUL_MUL: EthGas = EthGas::new(6_000);

    /// Cost of the Istanbul `alt_bn128_pair` per point.
    pub(super) const ISTANBUL_PAIR_PER_POINT: EthGas = EthGas::new(34_000);

    /// Cost of the Istanbul `alt_bn128_pair` operation.
    pub(super) const ISTANBUL_PAIR_BASE: EthGas = EthGas::new(45_000);
}

/// bn128 constants.
mod consts {
    /// Input length for the add operation.
    pub(super) const ADD_INPUT_LEN: usize = 128;

    /// Input length for the multiplication operation.
    pub(super) const MUL_INPUT_LEN: usize = 96;

    /// Pair element length.
    pub(super) const PAIR_ELEMENT_LEN: usize = 192;

    /// Size of BN scalars.
    pub(super) const SCALAR_LEN: usize = 32;

    /// Size of BN scalar for g2.
    pub(super) const SCALAR_2_LEN: usize = 64;
}

#[derive(Default)]
pub struct Bn256Add<HF: HardFork>(PhantomData<HF>);

impl<HF: HardFork> Bn256Add<HF> {
    pub const ADDRESS: Address = make_address(0, 6);

    #[must_use]
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<HF: HardFork> Bn256Add<HF> {
    fn run_inner(input: &[u8], _context: &Context) -> Result<Vec<u8>, ExitError> {
        let mut input = input.to_vec();
        input.resize(consts::ADD_INPUT_LEN, 0);
        input
            .chunks_mut(consts::SCALAR_LEN)
            .for_each(<[u8]>::reverse);
        let (left, input) = input.split_first_chunk().expect("valid constant");
        let (right, _) = input.split_first_chunk().expect("valid constant");
        let output = aurora_engine_sdk::alt_bn128_g1_sum(*left, *right).map_err(bn_error)?;

        Ok(output.to_vec())
    }
}

impl Precompile for Bn256Add<Byzantium> {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::BYZANTIUM_ADD)
    }

    /// Takes in two points on the elliptic curve `alt_bn128` and calculates the sum
    /// of them.
    ///
    /// See: `https://eips.ethereum.org/EIPS/eip-196`
    /// See: `https://etherscan.io/address/0000000000000000000000000000000000000006`
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
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

impl Precompile for Bn256Add<Istanbul> {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::ISTANBUL_ADD)
    }

    /// Takes in two points on the elliptic curve `alt_bn128` and calculates the sum
    /// of them.
    ///
    /// See: `https://eips.ethereum.org/EIPS/eip-196`
    /// See: `https://etherscan.io/address/0000000000000000000000000000000000000006`
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
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

#[derive(Default)]
pub struct Bn256Mul<HF: HardFork>(PhantomData<HF>);

impl<HF: HardFork> Bn256Mul<HF> {
    pub const ADDRESS: Address = make_address(0, 7);

    #[must_use]
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<HF: HardFork> Bn256Mul<HF> {
    fn run_inner(input: &[u8], _context: &Context) -> Result<Vec<u8>, ExitError> {
        let mut input = input.to_vec();
        input.resize(consts::MUL_INPUT_LEN, 0);
        input
            .chunks_mut(consts::SCALAR_LEN)
            .for_each(<[u8]>::reverse);
        let (point, input) = input.split_first_chunk().expect("valid constant");
        let (scalar, _) = input.split_first_chunk().expect("valid constant");
        let output =
            aurora_engine_sdk::alt_bn128_g1_scalar_multiple(*point, *scalar).map_err(bn_error)?;

        Ok(output.to_vec())
    }
}

impl Precompile for Bn256Mul<Byzantium> {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::BYZANTIUM_MUL)
    }

    /// Takes in two points on the elliptic curve `alt_bn128` and multiples them.
    ///
    /// See: `https://eips.ethereum.org/EIPS/eip-196`
    /// See: `https://etherscan.io/address/0000000000000000000000000000000000000007`
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
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

impl Precompile for Bn256Mul<Istanbul> {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::ISTANBUL_MUL)
    }

    /// Takes in two points on the elliptic curve `alt_bn128` and multiples them.
    ///
    /// See: `https://eips.ethereum.org/EIPS/eip-196`
    /// See: `https://etherscan.io/address/0000000000000000000000000000000000000007`
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
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

#[derive(Default)]
pub struct Bn256Pair<HF: HardFork>(PhantomData<HF>);

impl<HF: HardFork> Bn256Pair<HF> {
    pub const ADDRESS: Address = make_address(0, 8);

    #[must_use]
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<HF: HardFork> Bn256Pair<HF> {
    fn run_inner(input: &[u8], _context: &Context) -> Result<Vec<u8>, ExitError> {
        let res = if input.is_empty() {
            true
        } else {
            if input.len() % consts::PAIR_ELEMENT_LEN != 0 {
                return Err(ExitError::Other(Borrowed("ERR_BN128_INVALID_LEN")));
            }
            let pairs = input.chunks(consts::PAIR_ELEMENT_LEN).map(|element| {
                // `element.len() == consts::PAIR_ELEMENT_LEN` this is guaranteed by
                // `input.len() % consts::PAIR_ELEMENT_LEN == 0` guarded above

                let (g1, element) = element.split_first_chunk().expect("valid constant");
                let mut g1 = *g1;
                g1.chunks_mut(consts::SCALAR_LEN).for_each(<[u8]>::reverse);

                let (g2, _) = element.split_first_chunk().expect("valid constant");
                let mut g2 = *g2;
                g2.chunks_mut(consts::SCALAR_2_LEN)
                    .for_each(<[u8]>::reverse);

                (g1, g2)
            });

            aurora_engine_sdk::alt_bn128_pairing(pairs).map_err(|err| match err {
                BnError::Field(_) => ExitError::Other(Borrowed("ERR_FQ_INCORRECT")),
                BnError::Scalar(_) => ExitError::Other(Borrowed("ERR_BN128_INVALID_FR")),
                BnError::G1(_) => ExitError::Other(Borrowed("ERR_BN128_INVALID_A")),
                BnError::G2(_) => ExitError::Other(Borrowed("ERR_BN128_INVALID_B")),
            })?
        };

        let mut v = crate::vec![0u8; 32];
        if res {
            v[31] = 1;
        }
        Ok(v)
    }
}

impl Precompile for Bn256Pair<Byzantium> {
    fn required_gas(input: &[u8]) -> Result<EthGas, ExitError> {
        let input_len = u64::try_from(input.len()).map_err(utils::err_usize_conv)?;
        let pair_element_len = NonZeroUsize::try_from(consts::PAIR_ELEMENT_LEN)
            .and_then(NonZeroU64::try_from)
            .map_err(utils::err_usize_conv)?;
        Ok(
            costs::BYZANTIUM_PAIR_PER_POINT * input_len / pair_element_len
                + costs::BYZANTIUM_PAIR_BASE,
        )
    }

    /// Takes in elements and calculates the pair.
    ///
    /// See: `https://eips.ethereum.org/EIPS/eip-197`
    /// See: `https://etherscan.io/address/0000000000000000000000000000000000000008`
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
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

impl Precompile for Bn256Pair<Istanbul> {
    fn required_gas(input: &[u8]) -> Result<EthGas, ExitError> {
        let input_len = u64::try_from(input.len()).map_err(utils::err_usize_conv)?;
        let pair_element_len = NonZeroUsize::try_from(consts::PAIR_ELEMENT_LEN)
            .and_then(NonZeroU64::try_from)
            .map_err(utils::err_usize_conv)?;
        Ok(
            costs::ISTANBUL_PAIR_PER_POINT * input_len / pair_element_len
                + costs::ISTANBUL_PAIR_BASE,
        )
    }

    /// Takes in elements and calculates the pair.
    ///
    /// See: `https://eips.ethereum.org/EIPS/eip-197`
    /// See: `https://etherscan.io/address/0000000000000000000000000000000000000008`
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
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

#[allow(clippy::needless_pass_by_value)]
const fn bn_error(err: BnError) -> ExitError {
    match err {
        BnError::Field(_) => ExitError::Other(Borrowed("ERR_FQ_INCORRECT")),
        BnError::Scalar(_) => ExitError::Other(Borrowed("ERR_BN128_INVALID_FR")),
        BnError::G1(_) | BnError::G2(_) => ExitError::Other(Borrowed("ERR_BN128_INVALID_POINT")),
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

        let res = Bn256Add::<Byzantium>::new()
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

        let res = Bn256Add::<Byzantium>::new()
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
            Bn256Add::<Byzantium>::new().run(&input, Some(EthGas::new(499)), &new_context(), false);
        assert!(matches!(res, Err(ExitError::OutOfGas)));

        // no input test
        let input = [0u8; 0];
        let expected = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();

        let res = Bn256Add::<Byzantium>::new()
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
            Bn256Add::<Byzantium>::new().run(&input, Some(EthGas::new(500)), &new_context(), false);
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

        let res = Bn256Mul::<Byzantium>::new()
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
        let res = Bn256Mul::<Byzantium>::new().run(
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

        let res = Bn256Mul::<Byzantium>::new()
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

        let res = Bn256Mul::<Byzantium>::new()
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

        let res = Bn256Mul::<Byzantium>::new().run(
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
    #[allow(clippy::too_many_lines)]
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

        let res = Bn256Pair::<Byzantium>::new()
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
        let res = Bn256Pair::<Byzantium>::new().run(
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

        let res = Bn256Pair::<Byzantium>::new()
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

        let res = Bn256Pair::<Byzantium>::new().run(
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

        let res = Bn256Pair::<Byzantium>::new().run(
            &input,
            Some(EthGas::new(260_000)),
            &new_context(),
            false,
        );
        assert!(matches!(
            res,
            Err(ExitError::Other(Borrowed("ERR_BN128_INVALID_LEN",)))
        ));

        // on curve
        let input = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();
        let expected =
            hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();

        let res = Bn256Pair::<Byzantium>::new()
            .run(&input, Some(EthGas::new(260_000)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);
    }
}

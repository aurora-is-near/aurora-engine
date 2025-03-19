use super::G2_INPUT_ITEM_LENGTH;
use crate::prelude::{Borrowed, Vec};
use crate::{EvmPrecompileResult, Precompile, PrecompileOutput};
use aurora_engine_types::types::{make_address, Address, EthGas};
use evm::{Context, ExitError};

/// Base gas fee for BLS12-381 `g2_add` operation.
const BASE_GAS_FEE: u64 = 600;

/// Input length of `g2_add` operation.
const INPUT_LENGTH: usize = 512;

/// BLS12-381 G2 Add
pub struct BlsG2Add;

impl BlsG2Add {
    pub const ADDRESS: Address = make_address(0, 0xD);

    #[cfg(feature = "std")]
    fn execute(input: &[u8]) -> Result<Vec<u8>, ExitError> {
        use super::standalone::g2;
        use blst::{
            blst_p2, blst_p2_add_or_double_affine, blst_p2_affine, blst_p2_from_affine,
            blst_p2_to_affine,
        };

        // NB: There is no subgroup check for the G2 addition precompile.
        //
        // So we set the subgroup checks here to `false`
        let a_aff = &g2::extract_g2_input(&input[..G2_INPUT_ITEM_LENGTH], false)?;
        let b_aff = &g2::extract_g2_input(&input[G2_INPUT_ITEM_LENGTH..], false)?;

        let mut b = blst_p2::default();
        // SAFETY: b and b_aff are blst values.
        unsafe { blst_p2_from_affine(&mut b, b_aff) };

        let mut p = blst_p2::default();
        // SAFETY: p, b and a_aff are blst values.
        unsafe { blst_p2_add_or_double_affine(&mut p, &b, a_aff) };

        let mut p_aff = blst_p2_affine::default();
        // SAFETY: p_aff and p are blst values.
        unsafe { blst_p2_to_affine(&mut p_aff, &p) };

        Ok(g2::encode_g2_point(&p_aff))
    }

    #[cfg(not(feature = "std"))]
    #[allow(clippy::range_plus_one)]
    fn execute(input: &[u8]) -> Result<Vec<u8>, ExitError> {
        use super::{extract_g2, padding_g2_result, FP_LENGTH};

        let (p0_x, p0_y) = extract_g2(&input[..G2_INPUT_ITEM_LENGTH])?;
        let (p1_x, p1_y) = extract_g2(&input[G2_INPUT_ITEM_LENGTH..])?;

        let mut g2_input = [0u8; 8 * FP_LENGTH + 2];

        // Check zero input
        if input[..G2_INPUT_ITEM_LENGTH] == [0; G2_INPUT_ITEM_LENGTH] {
            g2_input[1] |= 0x40;
        } else {
            g2_input[1..1 + 2 * FP_LENGTH].copy_from_slice(&p0_x);
            g2_input[1 + 2 * FP_LENGTH..1 + 4 * FP_LENGTH].copy_from_slice(&p0_y);
        }

        if input[G2_INPUT_ITEM_LENGTH..] == [0; G2_INPUT_ITEM_LENGTH] {
            g2_input[2 + 4 * FP_LENGTH] |= 0x40;
        } else {
            g2_input[2 + 4 * FP_LENGTH..2 + 6 * FP_LENGTH].copy_from_slice(&p1_x);
            g2_input[2 + 6 * FP_LENGTH..2 + 8 * FP_LENGTH].copy_from_slice(&p1_y);
        }

        let output = aurora_engine_sdk::bls12381_p2_sum(&g2_input[..]);
        Ok(padding_g2_result(&output))
    }
}

impl Precompile for BlsG2Add {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError>
    where
        Self: Sized,
    {
        Ok(EthGas::new(BASE_GAS_FEE))
    }

    /// G2 addition call expects `512` bytes as an input that is interpreted as byte
    /// concatenation of two G2 points (`256` bytes each).
    ///
    /// Output is an encoding of addition operation result - single G2 point (`256`
    /// bytes).
    /// See also <https://eips.ethereum.org/EIPS/eip-2537#abi-for-g2-addition>
    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        _context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        let cost = Self::required_gas(input)?;
        if let Some(target_gas) = target_gas {
            if cost > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        if input.len() != INPUT_LENGTH {
            return Err(ExitError::Other(Borrowed("ERR_BLS_G2ADD_INPUT_LEN")));
        }

        let output = Self::execute(input)?;
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurora_engine_types::H160;

    #[test]
    fn bls12_381_g2_add() {
        let precompile = BlsG2Add;
        let ctx = Context {
            address: H160::zero(),
            caller: H160::zero(),
            apparent_value: 0.into(),
        };
        let input = hex::decode("\
               00000000000000000000000000000000161c595d151a765c7dee03c9210414cdffab84b9078b4b98f9df09be5ec299b8f6322c692214f00ede97958f235c352b\
			   00000000000000000000000000000000106883e0937cb869e579b513bde8f61020fcf26be38f8b98eae3885cedec2e028970415fc653cf10e64727b7f6232e06\
			   000000000000000000000000000000000f351a82b733af31af453904874b7ca6252957a1ab51ec7f7b6fff85bbf3331f870a7e72a81594a9930859237e7a154d\
			   0000000000000000000000000000000012fcf20d1750901f2cfed64fd362f010ee64fafe9ddab406cc352b65829b929881a50514d53247d1cca7d6995d0bc9b2\
			   00000000000000000000000000000000148b7dfc21521d79ff817c7a0305f1048851e283be13c07d5c04d28b571d48172838399ba539529e8d037ffd1f729558\
			   0000000000000000000000000000000003015abea326c15098f5205a8b2d3cd74d72dac59d60671ca6ef8c9c714ea61ffdacd46d1024b5b4f7e6b3b569fabaf2\
			   0000000000000000000000000000000011f0c512fe7dc2dd8abdc1d22c2ecd2e7d1b84f8950ab90fc93bf54badf7bb9a9bad8c355d52a5efb110dca891e4cc3d\
			   0000000000000000000000000000000019774010814d1d94caf3ecda3ef4f5c5986e966eaf187c32a8a5a4a59452af0849690cf71338193f2d8435819160bcfb")
            .expect("hex decoding failed");

        let res = precompile
            .run(&input, None, &ctx, false)
            .expect("precompile run should not fail");
        let expected = hex::decode("\
               000000000000000000000000000000000383ab7a17cc57e239e874af3f1aaabba0e64625b848676712f05f56132dbbd1cadfabeb3fe1f461daba3f1720057ddd\
			   00000000000000000000000000000000096967e9b3747f1b8e344535eaa0c51e70bc77412bfaa2a7ce76f11f570c9febb8f4227316866a416a50436d098e6f9a\
			   000000000000000000000000000000001079452b7519a7b090d668d54c266335b1cdd1080ed867dd17a2476b11c2617da829bf740e51cb7dfd60d73ed02c0c67\
			   00000000000000000000000000000000015fc3a972e05cbd9014882cfe6f2f16d0291c403bf28b05056ac625e4f71dfb1295c85d73145ef554614e6eb2d5bf02")
            .expect("hex decoding failed");

        assert_eq!(res.output, expected);
    }
}

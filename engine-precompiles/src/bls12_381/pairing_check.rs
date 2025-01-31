use super::{g1, g2};
use crate::prelude::Borrowed;
use crate::{utils, EvmPrecompileResult, Precompile, PrecompileOutput};
use aurora_engine_types::types::{make_address, Address, EthGas};
use blst::{blst_final_exp, blst_fp12, blst_fp12_is_one, blst_fp12_mul, blst_miller_loop};
use evm::{Context, ExitError};

/// Multiplier gas fee for BLS12-381 pairing operation.
const PAIRING_MULTIPLIER_BASE: u64 = 32600;
/// Offset gas fee for BLS12-381 pairing operation.
const PAIRING_OFFSET_BASE: u64 = 37700;
/// Input length of pairing operation.
const INPUT_LENGTH: usize = 384;

/// BLS12-382 Pairing check
pub struct BlsPairingCheck;

impl BlsPairingCheck {
    pub const ADDRESS: Address = make_address(0, 0xF);
}

impl Precompile for BlsPairingCheck {
    fn required_gas(input: &[u8]) -> Result<EthGas, ExitError>
    where
        Self: Sized,
    {
        let k = u64::try_from(input.len() / INPUT_LENGTH).map_err(utils::err_usize_conv)?;
        Ok(EthGas::new(
            PAIRING_MULTIPLIER_BASE * k + PAIRING_OFFSET_BASE,
        ))
    }

    /// Pairing call expects 384*k (k being a positive integer) bytes as an inputs
    /// that is interpreted as byte concatenation of k slices. Each slice has the
    /// following structure:
    ///    * 128 bytes of G1 point encoding
    ///    * 256 bytes of G2 point encoding
    ///
    /// Each point is expected to be in the subgroup of order q.
    /// Output is 32 bytes where first 31 bytes are equal to 0x00 and the last byte
    /// is 0x01 if pairing result is equal to the multiplicative identity in a pairing
    /// target field and 0x00 otherwise.
    ///
    /// See also: <https://eips.ethereum.org/EIPS/eip-2537#abi-for-pairing>
    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        _context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        let input_len = input.len();
        if input_len == 0 || input_len % INPUT_LENGTH != 0 {
            return Err(ExitError::Other(Borrowed("ERR_BLS_PAIRING_INVALID_LENGTH")));
        }

        let cost = Self::required_gas(input)?;
        if let Some(target_gas) = target_gas {
            if cost > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        let k = input_len / INPUT_LENGTH;
        // Accumulator for the fp12 multiplications of the miller loops.
        let mut acc = blst_fp12::default();
        for i in 0..k {
            // NB: Scalar multiplications, MSMs and pairings MUST perform a subgroup check.
            //
            // So we set the subgroup_check flag to `true`
            let p1_aff = &g1::extract_g1_input(
                &input[i * INPUT_LENGTH..i * INPUT_LENGTH + g1::G1_INPUT_ITEM_LENGTH],
                true,
            )?;

            // NB: Scalar multiplications, MSMs and pairings MUST perform a subgroup check.
            //
            // So we set the subgroup_check flag to `true`
            let p2_aff = &g2::extract_g2_input(
                &input[i * INPUT_LENGTH + g1::G1_INPUT_ITEM_LENGTH
                    ..i * INPUT_LENGTH + g1::G1_INPUT_ITEM_LENGTH + g2::G2_INPUT_ITEM_LENGTH],
                true,
            )?;

            if i > 0 {
                // After the first slice (i>0) we use cur_ml to store the current
                // miller loop and accumulate with the previous results using a fp12
                // multiplication.
                let mut cur_ml = blst_fp12::default();
                let mut res = blst_fp12::default();
                // SAFETY: res, acc, cur_ml, p1_aff and p2_aff are blst values.
                unsafe {
                    blst_miller_loop(&mut cur_ml, p2_aff, p1_aff);
                    blst_fp12_mul(&mut res, &acc, &cur_ml);
                }
                acc = res;
            } else {
                // On the first slice (i==0) there is no previous results and no need
                // to accumulate.
                // SAFETY: acc, p1_aff and p2_aff are blst values.
                unsafe {
                    blst_miller_loop(&mut acc, p2_aff, p1_aff);
                }
            }
        }

        // SAFETY: ret and acc are blst values.
        let mut ret = blst_fp12::default();
        unsafe {
            blst_final_exp(&mut ret, &acc);
        }

        let mut result: u8 = 0;
        // SAFETY: ret is a blst value.
        unsafe {
            if blst_fp12_is_one(&ret) {
                result = 1;
            }
        }
        let mut output = [0u8; 32];
        output[31] = result;
        Ok(PrecompileOutput::without_logs(cost, output.into()))
    }
}

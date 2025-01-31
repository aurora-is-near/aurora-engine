use super::remove_padding;
use super::{g2, PADDED_FP2_LENGTH, PADDED_FP_LENGTH};
use crate::prelude::types::{make_address, Address, EthGas};
use crate::prelude::Borrowed;
use crate::{EvmPrecompileResult, Precompile, PrecompileOutput};
use blst::{blst_map_to_g2, blst_p2, blst_p2_affine, blst_p2_to_affine};
use evm::{Context, ExitError};

/// Base gas fee for BLS12-381 `map_fp2_to_g2` operation.
const BASE_GAS_FEE: u64 = 23800;

/// BLS12-382 Map FP2 to G2
pub struct BlsMapFp2ToG2;

impl BlsMapFp2ToG2 {
    pub const ADDRESS: Address = make_address(0, 0x11);
}

impl Precompile for BlsMapFp2ToG2 {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError>
    where
        Self: Sized,
    {
        Ok(EthGas::new(BASE_GAS_FEE))
    }

    /// Field-to-curve call expects 128 bytes as an input that is interpreted as
    /// an element of Fp2. Output of this call is 256 bytes and is an encoded G2
    /// point.
    /// See also: <https://eips.ethereum.org/EIPS/eip-2537#abi-for-mapping-fp2-element-to-g2-point>
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

        if input.len() != PADDED_FP2_LENGTH {
            return Err(ExitError::Other(Borrowed("ERR_BLS_MAP_FP2_TO_G2_LEN")));
        }

        let input_p0_x = remove_padding(&input[..PADDED_FP_LENGTH])?;
        let input_p0_y = remove_padding(&input[PADDED_FP_LENGTH..PADDED_FP2_LENGTH])?;
        let fp2 = g2::check_canonical_fp2(input_p0_x, input_p0_y)?;

        let mut p = blst_p2::default();
        // SAFETY: p and fp2 are blst values.
        // third argument is unused if null.
        unsafe { blst_map_to_g2(&mut p, &fp2, core::ptr::null()) };

        let mut p_aff = blst_p2_affine::default();
        // SAFETY: p_aff and p are blst values.
        unsafe { blst_p2_to_affine(&mut p_aff, &p) };

        let output = g2::encode_g2_point(&p_aff);
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

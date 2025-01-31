use super::{g1, PADDED_FP_LENGTH};
use crate::bls12_381::{fp_from_bendian, remove_padding};
use crate::prelude::Borrowed;
use crate::{EvmPrecompileResult, Precompile, PrecompileOutput};
use aurora_engine_types::types::{make_address, Address, EthGas};
use blst::{blst_map_to_g1, blst_p1, blst_p1_affine, blst_p1_to_affine};
use evm::{Context, ExitError};

/// Base gas fee for BLS12-381 `map_fp_to_g1` operation.
const MAP_FP_TO_G1_BASE: u64 = 5500;

/// BLS12-382 Map FP to G1
pub struct BlsMapFpToG1;

impl BlsMapFpToG1 {
    pub const ADDRESS: Address = make_address(0, 0x10);
}

impl Precompile for BlsMapFpToG1 {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError>
    where
        Self: Sized,
    {
        Ok(EthGas::new(MAP_FP_TO_G1_BASE))
    }

    /// Field-to-curve call expects 64 bytes as an input that is interpreted as an
    /// element of Fp. Output of this call is 128 bytes and is an encoded G1 point.
    /// See also: <https://eips.ethereum.org/EIPS/eip-2537#abi-for-mapping-fp-element-to-g1-point>
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
        if input.len() != PADDED_FP_LENGTH {
            return Err(ExitError::Other(Borrowed("ERR_BLS_MAP_FP_TO_G1_LEN")));
        }

        let input_p0 = remove_padding(input)?;
        let fp = fp_from_bendian(input_p0)?;

        let mut p = blst_p1::default();
        // SAFETY: p and fp are blst values.
        // third argument is unused if null.
        unsafe { blst_map_to_g1(&mut p, &fp, core::ptr::null()) };

        let mut p_aff = blst_p1_affine::default();
        // SAFETY: p_aff and p are blst values.
        unsafe { blst_p1_to_affine(&mut p_aff, &p) };

        let output = g1::encode_g1_point(&p_aff);
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

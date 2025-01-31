use crate::bls12_381::g2;
use crate::prelude::Borrowed;
use crate::{EvmPrecompileResult, Precompile, PrecompileOutput};
use aurora_engine_types::types::{make_address, Address, EthGas};
use blst::{
    blst_p2, blst_p2_add_or_double_affine, blst_p2_affine, blst_p2_from_affine, blst_p2_to_affine,
};
use evm::{Context, ExitError};

/// Base gas fee for BLS12-381 `g2_add` operation.
const BASE_GAS_FEE: u64 = 600;

/// Input length of `g2_add` operation.
const INPUT_LENGTH: usize = 512;

/// BLS12-382 G2 Add
pub struct BlsG2Add;

impl BlsG2Add {
    pub const ADDRESS: Address = make_address(0, 0xD);
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

        // NB: There is no subgroup check for the G2 addition precompile.
        //
        // So we set the subgroup checks here to `false`
        let a_aff = &g2::extract_g2_input(&input[..g2::G2_INPUT_ITEM_LENGTH], false)?;
        let b_aff = &g2::extract_g2_input(&input[g2::G2_INPUT_ITEM_LENGTH..], false)?;

        let mut b = blst_p2::default();
        // SAFETY: b and b_aff are blst values.
        unsafe { blst_p2_from_affine(&mut b, b_aff) };

        let mut p = blst_p2::default();
        // SAFETY: p, b and a_aff are blst values.
        unsafe { blst_p2_add_or_double_affine(&mut p, &b, a_aff) };

        let mut p_aff = blst_p2_affine::default();
        // SAFETY: p_aff and p are blst values.
        unsafe { blst_p2_to_affine(&mut p_aff, &p) };

        let output = g2::encode_g2_point(&p_aff);
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

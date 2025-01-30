use super::g1;
use crate::prelude::types::{make_address, Address, EthGas};
use crate::prelude::Borrowed;
use crate::{EvmPrecompileResult, Precompile, PrecompileOutput};
use blst::{
    blst_p1, blst_p1_add_or_double_affine, blst_p1_affine, blst_p1_from_affine, blst_p1_to_affine,
};
use evm::{Context, ExitError};

/// Base gas fee for BLS12-381 `g1_add` operation.
const BASE_GAS_FEE: u64 = 375;

/// Input length of `g1_add` operation.
const INPUT_LENGTH: usize = 256;

/// BLS12-382 G1 Add
pub struct BlsG1Add;

impl BlsG1Add {
    pub const ADDRESS: Address = make_address(0, 0xB);
}

impl Precompile for BlsG1Add {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError>
    where
        Self: Sized,
    {
        Ok(EthGas::new(BASE_GAS_FEE))
    }

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
            return Err(ExitError::Other(Borrowed("ERR_BLS_G1ADD_INPUT_LEN")));
        }

        // NB: There is no subgroup check for the G1 addition precompile.
        //
        // We set the subgroup checks here to `false`
        let a_aff = &g1::extract_g1_input(&input[..g1::G1_INPUT_ITEM_LENGTH], false)?;
        let b_aff = &g1::extract_g1_input(&input[g1::G1_INPUT_ITEM_LENGTH..], false)?;

        let mut b = blst_p1::default();
        // SAFETY: b and b_aff are blst values.
        unsafe { blst_p1_from_affine(&mut b, b_aff) };

        let mut p = blst_p1::default();
        // SAFETY: p, b and a_aff are blst values.
        unsafe { blst_p1_add_or_double_affine(&mut p, &b, a_aff) };

        let mut p_aff = blst_p1_affine::default();
        // SAFETY: p_aff and p are blst values.
        unsafe { blst_p1_to_affine(&mut p_aff, &p) };

        let output = g1::encode_g1_point(&p_aff);
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

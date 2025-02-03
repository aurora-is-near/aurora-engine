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

    /// G1 addition call expects `256` bytes as an input that is interpreted as byte
    /// concatenation of two G1 points (`128` bytes each).
    /// Output is an encoding of addition operation result - single G1 point (`128`
    /// bytes).
    /// See also: <https://eips.ethereum.org/EIPS/eip-2537#abi-for-g1-addition>
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

#[cfg(test)]
mod tests {
    use super::*;
    use aurora_engine_types::H160;

    #[test]
    fn bls12_381_g1_add() {
        let precompile = BlsG1Add;
        let ctx = Context {
            address: H160::zero(),
            caller: H160::zero(),
            apparent_value: 0.into(),
        };
        let input = hex::decode("\
               00000000000000000000000000000000117dbe419018f67844f6a5e1b78a1e597283ad7b8ee7ac5e58846f5a5fd68d0da99ce235a91db3ec1cf340fe6b7afcdb\
			   0000000000000000000000000000000013316f23de032d25e912ae8dc9b54c8dba1be7cecdbb9d2228d7e8f652011d46be79089dd0a6080a73c82256ce5e4ed2\
			   000000000000000000000000000000000441e7f7f96198e4c23bd5eb16f1a7f045dbc8c53219ab2bcea91d3a027e2dfe659feac64905f8b9add7e4bfc91bec2b\
			   0000000000000000000000000000000005fc51bb1b40c87cd4292d4b66f8ca5ce4ef9abd2b69d4464b4879064203bda7c9fc3f896a3844ebc713f7bb20951d95")
            .expect("hex decoding failed");

        let res = precompile
            .run(&input, None, &ctx, false)
            .expect("precompile run should not fail");
        let expected = hex::decode("\
                0000000000000000000000000000000016b8ab56b45a9294466809b8e858c1ad15ad0d52cfcb62f8f5753dc94cee1de6efaaebce10701e3ec2ecaa9551024ea\
                600000000000000000000000000000000124571eec37c0b1361023188d66ec17c1ec230d31b515e0e81e599ec19e40c8a7c8cdea9735bc3d8b4e37ca7e5dd71f6")
            .expect("hex decoding failed");

        assert_eq!(res.output, expected);
    }
}

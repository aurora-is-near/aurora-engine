use super::PADDED_FP2_LENGTH;
use crate::prelude::types::{make_address, Address, EthGas};
use crate::prelude::{Borrowed, Vec};
use crate::{EvmPrecompileResult, Precompile, PrecompileOutput};
use aurora_evm::{Context, ExitError};

/// Base gas fee for BLS12-381 `map_fp2_to_g2` operation.
const BASE_GAS_FEE: u64 = 23800;

/// BLS12-381 Map FP2 to G2
pub struct BlsMapFp2ToG2;

impl BlsMapFp2ToG2 {
    pub const ADDRESS: Address = make_address(0, 0x11);

    #[cfg(feature = "std")]
    fn execute(input: &[u8]) -> Result<Vec<u8>, ExitError> {
        aurora_engine_sdk::bls12_381::map_fp2_to_g12(input)
            .map_err(|e| ExitError::Other(Borrowed(e.as_ref())))
    }

    #[cfg(not(feature = "std"))]
    fn execute(input: &[u8]) -> Result<Vec<u8>, ExitError> {
        use super::{padding_g2_result, remove_padding, FP_LENGTH, PADDED_FP_LENGTH};

        let mut p = [0; 2 * FP_LENGTH];
        let p1 = remove_padding(&input[..PADDED_FP_LENGTH])?;
        let p2 = remove_padding(&input[PADDED_FP_LENGTH..])?;
        p[..FP_LENGTH].copy_from_slice(p2);
        p[FP_LENGTH..].copy_from_slice(p1);

        let output = aurora_engine_sdk::bls12381_map_fp2_to_g2(&p[..]);
        Ok(padding_g2_result(&output))
    }
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

        let output = Self::execute(input)?;
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurora_engine_types::H160;

    #[test]
    fn bls12_381_fp2_to_g2() {
        let precompile = BlsMapFp2ToG2;
        let ctx = Context {
            address: H160::zero(),
            caller: H160::zero(),
            apparent_value: 0.into(),
        };
        let input = hex::decode("\
               000000000000000000000000000000000f470603a402bc134db1b389fd187460f9eb2dd001a2e99f730af386508c62f0e911d831a2562da84bce11d39f2ff13f\
			   000000000000000000000000000000000d8c45f4ab20642d0cba9764126e0818b7d731a6ba29ed234d9d6309a5e8ddfbd85193f1fa8b7cfeed3d31b23b904ee9")
            .expect("hex decoding failed");

        let res = precompile
            .run(&input, None, &ctx, false)
            .expect("precompile run should not fail");
        let expected = hex::decode("\
               0000000000000000000000000000000012e74d5a0c005a86ca148e9eff8e34a00bfa8b6e6aadf633d65cd09bb29917e0ceb0d5c9d9650c162d7fe4aa27452685\
			   0000000000000000000000000000000005f09101a2088712619f9c096403b66855a12f9016c55aef6047372fba933f02d9d59db1a86df7be57978021e2457821\
			   00000000000000000000000000000000136975b37fe400d1d217a2b496c1552b39be4e9e71dd7ad482f5f0836d271d02959fdb698dda3d0530587fb86e0db1dd\
			   0000000000000000000000000000000000bad0aabd9309e92e2dd752f4dd73be07c0de2c5ddd57916b9ffa065d7440d03d44e7c042075cda694414a9fb639bb7")
            .expect("hex decoding failed");

        assert_eq!(res.output, expected);
    }
}

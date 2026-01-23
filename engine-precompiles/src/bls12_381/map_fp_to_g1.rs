use aurora_engine_sdk::bls12_381::{self, PADDED_FP_LENGTH};
use aurora_engine_types::types::{Address, EthGas, make_address};
use aurora_evm::{Context, ExitError};

use crate::prelude::{Borrowed, Vec};
use crate::{EvmPrecompileResult, Precompile, PrecompileOutput};

/// Base gas fee for BLS12-381 `map_fp_to_g1` operation.
const MAP_FP_TO_G1_BASE: u64 = 5500;

/// BLS12-381 Map FP to G1
pub struct BlsMapFpToG1;

impl BlsMapFpToG1 {
    pub const ADDRESS: Address = make_address(0, 0x10);

    fn execute(input: &[u8]) -> Result<Vec<u8>, ExitError> {
        bls12_381::map_fp_to_g1(input).map_err(|e| ExitError::Other(Borrowed(e.as_ref())))
    }
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

        let output = Self::execute(input)?;
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurora_engine_types::H160;

    #[test]
    fn bls12_381_fp_to_g1() {
        let precompile = BlsMapFpToG1;
        let ctx = Context {
            address: H160::zero(),
            caller: H160::zero(),
            apparent_value: 0.into(),
        };
        let input = hex::decode("0000000000000000000000000000000017f66b472b36717ee0902d685c808bb5f190bbcb2c51d067f1cbec64669f10199a5868d7181dcec0498fcc71f5acaf79").expect("hex decoding failed");

        let res = precompile
            .run(&input, None, &ctx, false)
            .expect("precompile run should not fail");
        let expected = hex::decode("\
               00000000000000000000000000000000188dc9e5ddf48977f33aeb6e505518269bf67fb624fa86b79741d842e75a6fa1be0911c2caa9e55571b6e55a3c0c0b9e\
			   00000000000000000000000000000000193e8b7c7e78daf104a59d7b39401a65355fa874bd34e91688580941e99a863367efc68fe871e38e07423090e93919c9")
            .expect("hex decoding failed");

        assert_eq!(res.output, expected);
    }
}

use crate::prelude::{Borrowed, Vec};
use crate::{utils, EvmPrecompileResult, Precompile, PrecompileOutput};
use aurora_engine_sdk::bls12_381::{self, PAIRING_INPUT_LENGTH};
use aurora_engine_types::types::{make_address, Address, EthGas};
use aurora_evm::{Context, ExitError};

/// Multiplier gas fee for BLS12-381 pairing operation.
const PAIRING_MULTIPLIER_BASE: u64 = 32600;
/// Offset gas fee for BLS12-381 pairing operation.
const PAIRING_OFFSET_BASE: u64 = 37700;

/// BLS12-381 Pairing check
pub struct BlsPairingCheck;

impl BlsPairingCheck {
    pub const ADDRESS: Address = make_address(0, 0xF);

    fn execute(input: &[u8]) -> Result<Vec<u8>, ExitError> {
        bls12_381::pairing_check(input).map_err(|e| ExitError::Other(Borrowed(e.as_ref())))
    }
}

impl Precompile for BlsPairingCheck {
    fn required_gas(input: &[u8]) -> Result<EthGas, ExitError>
    where
        Self: Sized,
    {
        let k = u64::try_from(input.len() / PAIRING_INPUT_LENGTH).map_err(utils::err_usize_conv)?;
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
        if input_len == 0 || input_len % PAIRING_INPUT_LENGTH != 0 {
            return Err(ExitError::Other(Borrowed("ERR_BLS_PAIRING_INVALID_LENGTH")));
        }

        let cost = Self::required_gas(input)?;
        if let Some(target_gas) = target_gas {
            if cost > target_gas {
                return Err(ExitError::OutOfGas);
            }
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
    fn bls12_381_pairing() {
        let precompile = BlsPairingCheck;
        let ctx = Context {
            address: H160::zero(),
            caller: H160::zero(),
            apparent_value: 0.into(),
        };
        let input = hex::decode("\
			   000000000000000000000000000000001830f52d9bff64a623c6f5259e2cd2c2a08ea17a8797aaf83174ea1e8c3bd3955c2af1d39bfa474815bfe60714b7cd80\
			   000000000000000000000000000000000874389c02d4cf1c61bc54c4c24def11dfbe7880bc998a95e70063009451ee8226fec4b278aade3a7cea55659459f1d5\
			   00000000000000000000000000000000197737f831d4dc7e708475f4ca7ca15284db2f3751fcaac0c17f517f1ddab35e1a37907d7b99b39d6c8d9001cd50e79e\
			   000000000000000000000000000000000af1a3f6396f0c983e7c2d42d489a3ae5a3ff0a553d93154f73ac770cd0af7467aa0cef79f10bbd34621b3ec9583a834\
			   000000000000000000000000000000001918cb6e448ed69fb906145de3f11455ee0359d030e90d673ce050a360d796de33ccd6a941c49a1414aca1c26f9e699e\
			   0000000000000000000000000000000019a915154a13249d784093facc44520e7f3a18410ab2a3093e0b12657788e9419eec25729944f7945e732104939e7a9e\
			   000000000000000000000000000000001830f52d9bff64a623c6f5259e2cd2c2a08ea17a8797aaf83174ea1e8c3bd3955c2af1d39bfa474815bfe60714b7cd80\
			   00000000000000000000000000000000118cd94e36ab177de95f52f180fdbdc584b8d30436eb882980306fa0625f07a1f7ad3b4c38a921c53d14aa9a6ba5b8d6\
			   00000000000000000000000000000000197737f831d4dc7e708475f4ca7ca15284db2f3751fcaac0c17f517f1ddab35e1a37907d7b99b39d6c8d9001cd50e79e\
			   000000000000000000000000000000000af1a3f6396f0c983e7c2d42d489a3ae5a3ff0a553d93154f73ac770cd0af7467aa0cef79f10bbd34621b3ec9583a834\
			   000000000000000000000000000000001918cb6e448ed69fb906145de3f11455ee0359d030e90d673ce050a360d796de33ccd6a941c49a1414aca1c26f9e699e\
			   0000000000000000000000000000000019a915154a13249d784093facc44520e7f3a18410ab2a3093e0b12657788e9419eec25729944f7945e732104939e7a9e")
            .expect("hex decoding failed");

        let res = precompile
            .run(&input, None, &ctx, false)
            .expect("precompile run should not fail");
        let expected =
            hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .expect("hex decoding failed");

        assert_eq!(res.output, expected);
    }
}

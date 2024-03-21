use crate::{EvmPrecompileResult, Precompile, PrecompileOutput};
use aurora_engine_types::types::{Address, EthGas};
use evm::{Context, ExitError};

pub struct Identity;

impl Identity {
    pub const ADDRESS: Address = crate::identity::ADDRESS;
}

impl Precompile for Identity {
    fn required_gas(input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(EthGas::new(
            crate::identity::required_gas(input).map_err(Into::<ExitError>::into)?,
        ))
    }

    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        _context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        let gas_limit = target_gas.unwrap_or(EthGas::new(u64::MAX));
        let (gas_used, input_data) =
            crate::identity::run(input, gas_limit.as_u64()).map_err(Into::<ExitError>::into)?;
        Ok(PrecompileOutput::without_logs(
            EthGas::new(gas_used),
            input_data,
        ))
    }
}

#[cfg(test)]
mod tests {
    use evm::ExitError;

    use crate::utils::new_context;

    use super::*;

    #[test]
    fn test_identity() {
        let input = [0u8, 1, 2, 3];

        let expected = input[0..2].to_vec();
        let res = Identity
            .run(&input[0..2], Some(EthGas::new(18)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        let expected = input.to_vec();
        let res = Identity
            .run(&input, Some(EthGas::new(18)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);

        // gas fail
        let res = Identity.run(&input[0..2], Some(EthGas::new(17)), &new_context(), false);

        assert!(matches!(res, Err(ExitError::OutOfGas)));

        // larger input
        let input = [
            0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31, 32,
        ];
        let res = Identity
            .run(&input, Some(EthGas::new(21)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, input.to_vec());
    }
}

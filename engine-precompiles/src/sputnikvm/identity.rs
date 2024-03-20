use crate::identity::Identity;
use crate::{utils, EvmPrecompileResult, Precompile, PrecompileOutput};
use alloc::borrow::Cow::Borrowed;
use aurora_engine_types::types::{make_address, Address, EthGas};
use evm::{Context, ExitError};

pub struct Identity;

impl Identity {
    pub const ADDRESS: Address = crate::identity::ADDERESS;
}

impl Precompile for Identity {
    fn required_gas(input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(EthGas::new(
            crate::identity::required_gas(input).map_err(Into::into)?,
        ))
    }

    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        _context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        let gas_limit = target_gas.ok_or(ExitError::Other(Borrowed("ERR_GAS_LIMIT_NOT_SET")))?;
        let (gas_used, input_data) =
            crate::identity::run(input, gas_limit.as_u64()).map_err(Into::into)?;
        Ok(PrecompileOutput::without_logs(
            EthGas::new(gas_used),
            input_data,
        ))
    }
}

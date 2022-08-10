use super::{EvmPrecompileResult, Precompile};
use crate::prelude::types::{Address, EthGas};
use crate::PrecompileOutput;
use aurora_engine_sdk::promise::ReadOnlyPromiseHandler;
use aurora_engine_types::{Cow, Vec};
use borsh::BorshSerialize;
use evm::{Context, ExitError};

/// predecessor_account_id precompile address
///
/// Address: `0x0a3540f79be10ef14890e87c1a0040a68cc6af71`
/// This address is computed as: `&keccak("prepaidGas")[12..]`
pub const ADDRESS: Address = crate::make_address(0x0a3540f7, 0x9be10ef14890e87c1a0040a68cc6af71);

mod costs {
    use crate::prelude::types::EthGas;

    // TODO(#483): Determine the correct amount of gas
    pub(super) const PROMISE_RESULT_GAS_COST: EthGas = EthGas::new(0);
}

pub struct PromiseResult<H> {
    handler: H,
}

impl<H> PromiseResult<H> {
    pub fn new(handler: H) -> Self {
        Self { handler }
    }
}

impl<H: ReadOnlyPromiseHandler> Precompile for PromiseResult<H> {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::PROMISE_RESULT_GAS_COST)
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

        let num_promises = self.handler.ro_promise_results_count();
        let n_usize = usize::try_from(num_promises).map_err(crate::utils::err_usize_conv)?;
        let mut results = Vec::with_capacity(n_usize);
        for i in 0..num_promises {
            if let Some(result) = self.handler.ro_promise_result(i) {
                results.push(result);
            }
        }
        let bytes = results
            .try_to_vec()
            .map_err(|_| ExitError::Other(Cow::Borrowed("ERR_PROMISE_RESULT_SERIALIZATION")))?;
        Ok(PrecompileOutput::without_logs(cost, bytes))
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::sdk::types::near_account_to_evm_address;
    use crate::promise_result;

    #[test]
    fn test_get_promise_results_precompile_id() {
        assert_eq!(
            promise_result::ADDRESS,
            near_account_to_evm_address("getPromiseResults".as_bytes())
        );
    }
}

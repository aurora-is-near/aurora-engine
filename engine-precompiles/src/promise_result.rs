use super::{EvmPrecompileResult, Precompile};
use crate::prelude::types::{Address, EthGas};
use crate::PrecompileOutput;
use aurora_engine_sdk::promise::ReadOnlyPromiseHandler;
use aurora_engine_types::{Cow, Vec};
use borsh::BorshSerialize;
use evm::{Context, ExitError};

/// get_promise_results precompile address
///
/// Address: `0x0a3540f79be10ef14890e87c1a0040a68cc6af71`
/// This address is computed as: `&keccak("getPromiseResults")[12..]`
pub const ADDRESS: Address = crate::make_address(0x0a3540f7, 0x9be10ef14890e87c1a0040a68cc6af71);

pub mod costs {
    use crate::prelude::types::EthGas;

    /// This cost is always charged for calling this precompile.
    pub const PROMISE_RESULT_BASE_COST: EthGas = EthGas::new(105);
    /// This is the cost per byte of promise result data.
    pub const PROMISE_RESULT_BYTE_COST: EthGas = EthGas::new(1);
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
        // Only gives the cost we can know without reading any promise data.
        // This allows failing fast in the case the base cost cannot even be covered.
        Ok(costs::PROMISE_RESULT_BASE_COST)
    }

    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        _context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        let mut cost = Self::required_gas(input)?;
        let check_cost = |cost: EthGas| -> Result<(), ExitError> {
            if let Some(target_gas) = target_gas {
                if cost > target_gas {
                    return Err(ExitError::OutOfGas);
                }
            }
            Ok(())
        };
        check_cost(cost)?;

        let num_promises = self.handler.ro_promise_results_count();
        let n_usize = usize::try_from(num_promises).map_err(crate::utils::err_usize_conv)?;
        let mut results = Vec::with_capacity(n_usize);
        for i in 0..num_promises {
            if let Some(result) = self.handler.ro_promise_result(i) {
                let n_bytes = u64::try_from(result.size()).map_err(crate::utils::err_usize_conv)?;
                cost += n_bytes * costs::PROMISE_RESULT_BYTE_COST;
                check_cost(cost)?;
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

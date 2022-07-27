//! Cross contract call precompile.
//!
//! Allow Aurora users interacting with NEAR smart contracts using cross contract call primitives.
//! TODO: How they work (low level explanation with examples)

use crate::{Context, EvmPrecompileResult, Precompile, PrecompileOutput};
use aurora_engine_sdk::io::IO;
use aurora_engine_types::{types::EthGas, vec, Cow};
use evm_core::ExitError;

mod costs {
    use crate::prelude::types::EthGas;

    // TODO(#483): Determine the correct amount of gas
    pub(super) const CROSS_CONTRACT_CALL: EthGas = EthGas::new(0);
}

pub struct CrossContractCall<I> {
    io: I,
}

impl<I> CrossContractCall<I> {
    pub fn new(io: I) -> Self {
        Self { io }
    }
}

pub mod cross_contract_call {
    use aurora_engine_types::types::Address;

    /// Exit to Ethereum precompile address
    ///
    /// Address: `0x516cded1d16af10cad47d6d49128e2eb7d27b372`
    /// This address is computed as: `&keccak("nearCrossContractCall")[12..]`
    pub const ADDRESS: Address =
        crate::make_address(0x516cded1, 0xd16af10cad47d6d49128e2eb7d27b372);
}

impl<I: IO> Precompile for CrossContractCall<I> {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::CROSS_CONTRACT_CALL)
    }

    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        context: &Context,
        is_static: bool,
    ) -> EvmPrecompileResult {
        if let Some(target_gas) = target_gas {
            if Self::required_gas(input)? > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        // It's not allowed to call cross contract call precompile in static or delegate mode
        if is_static {
            return Err(ExitError::Other(Cow::from("ERR_INVALID_IN_STATIC")));
        } else if context.address != cross_contract_call::ADDRESS.raw() {
            return Err(ExitError::Other(Cow::from("ERR_INVALID_IN_DELEGATE")));
        }

        Ok(PrecompileOutput {
            logs: vec![],
            ..Default::default()
        }
        .into())
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::sdk::types::near_account_to_evm_address;
    use crate::xcc::cross_contract_call;

    #[test]
    fn test_precompile_id() {
        assert_eq!(
            cross_contract_call::ADDRESS,
            near_account_to_evm_address("nearCrossContractCall".as_bytes())
        );
    }
}

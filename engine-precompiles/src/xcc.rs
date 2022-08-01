//! Cross contract call precompile.
//!
//! Allow Aurora users interacting with NEAR smart contracts using cross contract call primitives.
//! TODO: How they work (low level explanation with examples)

use crate::{Context, EvmPrecompileResult, Precompile, PrecompileOutput};
use aurora_engine_sdk::io::IO;
use aurora_engine_types::{
    account_id::AccountId,
    format,
    parameters::{CrossContractCallArgs, PromiseArgs, PromiseCreateArgs},
    types::{balance::ZERO_YOCTO, EthGas},
    vec, Cow, Vec, H160,
};
use borsh::{BorshDeserialize, BorshSerialize};
use evm::backend::Log;
use evm_core::ExitError;

const ERR_INVALID_INPUT: &str = "ERR_INVALID_XCC_INPUT";
const ERR_SERIALIZE: &str = "ERR_XCC_CALL_SERIALIZE";
const ERR_STATIC: &str = "ERR_INVALID_IN_STATIC";
const ERR_DELEGATE: &str = "ERR_INVALID_IN_DELEGATE";
const ROUTER_EXEC_NAME: &str = "execute";
const ROUTER_SCHEDULE_NAME: &str = "schedule";

pub mod costs {
    use crate::prelude::types::{EthGas, NearGas};

    // TODO(#483): Determine the correct amount of gas
    pub(super) const CROSS_CONTRACT_CALL: EthGas = EthGas::new(0);

    pub const ROUTER_EXEC: NearGas = NearGas::new(7_000_000_000_000);
    pub const ROUTER_SCHEDULE: NearGas = NearGas::new(5_000_000_000_000);
}

pub struct CrossContractCall<I> {
    io: I,
    engine_account_id: AccountId,
}

impl<I> CrossContractCall<I> {
    pub fn new(engine_account_id: AccountId, io: I) -> Self {
        Self {
            io,
            engine_account_id,
        }
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
            return Err(ExitError::Other(Cow::from(ERR_STATIC)));
        } else if context.address != cross_contract_call::ADDRESS.raw() {
            return Err(ExitError::Other(Cow::from(ERR_DELEGATE)));
        }

        let sender = context.caller;
        let target_account_id = create_target_account_id(sender, self.engine_account_id.as_ref());
        // TODO: Is it ok to use Borsh to read the input? It might not be very friendly to construct the input in Solidity...
        let args = CrossContractCallArgs::try_from_slice(input)
            .map_err(|_| ExitError::Other(Cow::from(ERR_INVALID_INPUT)))?;
        let promise = match args {
            CrossContractCallArgs::Eager(call) => {
                let call_gas = match &call {
                    PromiseArgs::Create(call) => call.attached_gas,
                    PromiseArgs::Callback(cb) => cb.base.attached_gas + cb.callback.attached_gas,
                };
                PromiseCreateArgs {
                    target_account_id,
                    method: ROUTER_EXEC_NAME.into(),
                    args: call
                        .try_to_vec()
                        .map_err(|_| ExitError::Other(Cow::from(ERR_SERIALIZE)))?,
                    attached_balance: ZERO_YOCTO,
                    attached_gas: costs::ROUTER_EXEC + call_gas,
                }
            }
            CrossContractCallArgs::Delayed(call) => PromiseCreateArgs {
                target_account_id,
                method: ROUTER_SCHEDULE_NAME.into(),
                args: call
                    .try_to_vec()
                    .map_err(|_| ExitError::Other(Cow::from(ERR_SERIALIZE)))?,
                attached_balance: ZERO_YOCTO,
                // We don't need to add any gas to the amount need for the schedule call
                // since the promise is not executed right away.
                attached_gas: costs::ROUTER_SCHEDULE,
            },
        };

        let promise_log = Log {
            address: cross_contract_call::ADDRESS.raw(),
            topics: Vec::new(),
            data: promise
                .try_to_vec()
                .map_err(|_| ExitError::Other(Cow::from(ERR_SERIALIZE)))?,
        };

        Ok(PrecompileOutput {
            logs: vec![promise_log],
            ..Default::default()
        }
        .into())
    }
}

fn create_target_account_id(sender: H160, engine_account_id: &str) -> AccountId {
    format!("{}.{}", hex::encode(sender.as_bytes()), engine_account_id)
        .parse()
        .unwrap()
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

//! Cross contract call precompile.
//!
//! Allow Aurora users interacting with NEAR smart contracts using cross contract call primitives.
//! TODO: How they work (low level explanation with examples)

use crate::{Context, EvmPrecompileResult, Precompile, PrecompileOutput};
use aurora_engine_sdk::io::IO;
use aurora_engine_types::{
    account_id::AccountId,
    format,
    parameters::{CrossContractCallArgs, PromiseCreateArgs},
    types::{balance::ZERO_YOCTO, EthGas},
    vec, Cow, Vec, H160,
};
use borsh::{BorshDeserialize, BorshSerialize};
use evm::backend::Log;
use evm_core::ExitError;

pub mod costs {
    use crate::prelude::types::{EthGas, NearGas};

    /// Base EVM gas cost for calling this precompile.
    pub const CROSS_CONTRACT_CALL_BASE: EthGas = EthGas::new(115_000);
    /// Additional EVM gas cost per bytes of input given.
    pub const CROSS_CONTRACT_CALL_BYTE: EthGas = EthGas::new(2);
    /// EVM gas cost per NEAR gas attached to the created promise.
    /// Derived from the gas report https://hackmd.io/@birchmd/Sy4piXQ29
    /// The units on this quantity are `NEAR Gas / EVM Gas`
    pub const CROSS_CONTRACT_CALL_NEAR_GAS: u64 = 175_000_000;

    pub const ROUTER_EXEC: NearGas = NearGas::new(7_000_000_000_000);
    pub const ROUTER_SCHEDULE: NearGas = NearGas::new(5_000_000_000_000);
}

mod consts {
    pub(super) const ERR_INVALID_INPUT: &str = "ERR_INVALID_XCC_INPUT";
    pub(super) const ERR_SERIALIZE: &str = "ERR_XCC_CALL_SERIALIZE";
    pub(super) const ERR_STATIC: &str = "ERR_INVALID_IN_STATIC";
    pub(super) const ERR_DELEGATE: &str = "ERR_INVALID_IN_DELEGATE";
    pub(super) const ROUTER_EXEC_NAME: &str = "execute";
    pub(super) const ROUTER_SCHEDULE_NAME: &str = "schedule";
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
    fn required_gas(input: &[u8]) -> Result<EthGas, ExitError> {
        // This only includes the cost we can easily derive without parsing the input.
        // The other cost is added in later to avoid parsing the input more than once.
        Ok(costs::CROSS_CONTRACT_CALL_BASE + costs::CROSS_CONTRACT_CALL_BYTE * input.len())
    }

    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        context: &Context,
        is_static: bool,
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

        // It's not allowed to call cross contract call precompile in static or delegate mode
        if is_static {
            return Err(ExitError::Other(Cow::from(consts::ERR_STATIC)));
        } else if context.address != cross_contract_call::ADDRESS.raw() {
            return Err(ExitError::Other(Cow::from(consts::ERR_DELEGATE)));
        }

        let sender = context.caller;
        let target_account_id = create_target_account_id(sender, self.engine_account_id.as_ref());
        let args = CrossContractCallArgs::try_from_slice(input)
            .map_err(|_| ExitError::Other(Cow::from(consts::ERR_INVALID_INPUT)))?;
        let promise = match args {
            CrossContractCallArgs::Eager(call) => {
                let call_gas = call.total_gas();
                PromiseCreateArgs {
                    target_account_id,
                    method: consts::ROUTER_EXEC_NAME.into(),
                    args: call
                        .try_to_vec()
                        .map_err(|_| ExitError::Other(Cow::from(consts::ERR_SERIALIZE)))?,
                    attached_balance: ZERO_YOCTO,
                    attached_gas: costs::ROUTER_EXEC + call_gas,
                }
            }
            CrossContractCallArgs::Delayed(call) => PromiseCreateArgs {
                target_account_id,
                method: consts::ROUTER_SCHEDULE_NAME.into(),
                args: call
                    .try_to_vec()
                    .map_err(|_| ExitError::Other(Cow::from(consts::ERR_SERIALIZE)))?,
                attached_balance: ZERO_YOCTO,
                // We don't need to add any gas to the amount need for the schedule call
                // since the promise is not executed right away.
                attached_gas: costs::ROUTER_SCHEDULE,
            },
        };
        cost += EthGas::new(promise.attached_gas.as_u64() / costs::CROSS_CONTRACT_CALL_NEAR_GAS);
        check_cost(cost)?;

        let promise_log = Log {
            address: cross_contract_call::ADDRESS.raw(),
            topics: Vec::new(),
            data: promise
                .try_to_vec()
                .map_err(|_| ExitError::Other(Cow::from(consts::ERR_SERIALIZE)))?,
        };

        Ok(PrecompileOutput {
            logs: vec![promise_log],
            cost,
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

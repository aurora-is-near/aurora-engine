use super::{EvmPrecompileResult, Precompile};
use crate::prelude::types::{Address, EthGas};
use crate::prelude::{vec, Vec};
use crate::PrecompileOutput;
use aurora_engine_sdk::types::near_account_to_evm_address;
use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::{CallArgs, FunctionCallArgsV2};
use aurora_engine_types::types::WeiU256;
use aurora_engine_types::Cow;
use borsh::BorshSerialize;
use evm::backend::Log;
use evm::{Context, ExitError};

const ERR_INVALID_INPUT: &str = "ERR_INVALID_ROUTER_INPUT";
mod costs {
    use crate::prelude::types::EthGas;

    // TODO(#483): Determine the correct amount of gas
    pub(super) const ASYNC_ROUTER_GAS: EthGas = EthGas::new(0);
}

pub struct AsyncRouter;

/// async_router precompile address
///
/// Address: `0xad65a767211ae644cdf2d036853e2bcda225dff8`
/// This address is computed as: `&keccak("asyncRouter")[12..]`
pub const ADDRESS: Address = crate::make_address(0xad65a767, 0x211ae644cdf2d036853e2bcda225dff8);

fn predecessor_address(predecessor_account_id: &AccountId) -> Address {
    near_account_to_evm_address(predecessor_account_id.as_bytes())
}

impl Precompile for AsyncRouter {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::ASYNC_ROUTER_GAS)
    }

    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        let cost = Self::required_gas(input)?;
        if let Some(target_gas) = target_gas {
            if cost > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        let mut tokens = ethabi::decode(
            &[
                ethabi::ParamType::Address,
                ethabi::ParamType::Bytes,
                ethabi::ParamType::Bool,
            ],
            input,
        )
        .map_err(|_| ExitError::Other(Cow::from(ERR_INVALID_INPUT)))?;

        let attach_msg_sender = tokens
            .pop()
            .ok_or_else(|| ExitError::Other(Cow::from(ERR_INVALID_INPUT)))?
            .into_bool()
            .ok_or_else(|| ExitError::Other(Cow::from(ERR_INVALID_INPUT)))?;

        let mut payload = tokens
            .pop()
            .ok_or_else(|| ExitError::Other(Cow::from(ERR_INVALID_INPUT)))?
            .into_bytes()
            .ok_or_else(|| ExitError::Other(Cow::from(ERR_INVALID_INPUT)))?;

        let contract = Address::new(
            tokens
                .pop()
                .ok_or_else(|| ExitError::Other(Cow::from(ERR_INVALID_INPUT)))?
                .into_address()
                .ok_or_else(|| ExitError::Other(Cow::from(ERR_INVALID_INPUT)))?,
        );

        if attach_msg_sender {
            payload.extend_from_slice(context.caller.as_bytes())
        }

        let args = CallArgs::V2(FunctionCallArgsV2 {
            contract,
            value: WeiU256::from(context.apparent_value),
            input: payload,
        });

        let call_event_log = Log {
            address: ADDRESS.raw(),
            topics: Vec::new(),
            data: args.try_to_vec().unwrap(),
        };

        Ok(PrecompileOutput {
            logs: vec![call_event_log],
            ..Default::default()
        }
        .into())
    }
}

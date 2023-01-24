use crate::prelude::{vec, Cow};
use crate::set_gas_token::events::SetGasTokenLog;
use crate::{EvmPrecompileResult, Precompile, PrecompileOutput};
use aurora_engine_types::types::{Address, EthGas};
use evm::backend::Log;
use evm::{Context, ExitError};

pub use consts::SET_GAS_TOKEN_ADDRESS;

mod costs {
    use crate::prelude::types::EthGas;

    // TODO: gas costs, could be calculated returning logs of NEAR gas used prior and after.
    // Should check if the gas check adds gas itself as well..?
    pub(super) const SET_GAS_TOKEN_GAS: EthGas = EthGas::new(0);
}

pub mod consts {
    use aurora_engine_types::types::Address;

    /// Change gas token precompile address.
    ///
    /// Address: `0x076dae45c8e16a92258252fe04dedd97f1ea93d6`
    ///
    /// This address is computed as: `keccak("setGasToken")[12..]`
    pub const SET_GAS_TOKEN_ADDRESS: Address =
        crate::make_address(0x076dae45, 0xc8e16a92258252fe04dedd97f1ea93d6);
}

pub mod events {
    use crate::prelude::vec;
    use crate::set_gas_token::consts;
    use aurora_engine_types::types::Address;
    use aurora_engine_types::H256;
    use evm::backend::Log;

    // TODO
    pub(crate) const SET_GAS_TOKEN_SIGNATURE: H256 = crate::make_h256(
        0x29d0b6eaa171d0d1607729f506329510,
        0x7bc9766ba17d250f129cb5bd06503d13,
    );

    pub(crate) struct SetGasTokenLog {
        pub sender: Address,
        pub gas_token: Address,
    }

    impl SetGasTokenLog {
        pub(crate) fn encode(self) -> Log {
            let data = ethabi::encode(&[ethabi::Token::Address(self.gas_token.raw())]);
            let sender_address = {
                let mut buf = [0u8; 32];
                buf[12..].copy_from_slice(self.sender.as_bytes());
                H256(buf)
            };
            let topics = vec![SET_GAS_TOKEN_SIGNATURE, sender_address];

            let raw_log = ethabi::RawLog { topics, data };

            Log {
                address: consts::SET_GAS_TOKEN_ADDRESS.raw(),
                topics: raw_log.topics,
                data: raw_log.data,
            }
        }
    }

    #[cfg(test)]
    pub fn set_gas_token_schema() -> ethabi::Event {
        ethabi::Event {
            name: "SetGasToken".into(),
            inputs: vec![
                ethabi::EventParam {
                    name: "sender".into(),
                    kind: ethabi::ParamType::Address,
                    indexed: true,
                },
                ethabi::EventParam {
                    name: "gas_token".into(),
                    kind: ethabi::ParamType::Address,
                    indexed: true,
                },
            ],
            anonymous: false,
        }
    }
}

/// A precompile contract used to set the gas token.
///
/// Takes an input which must be an approved ERC-20 contract, or ETH itself at
/// the address `0x0`.
pub struct SetGasToken;

impl SetGasToken {
    pub const ADDRESS: Address = SET_GAS_TOKEN_ADDRESS;
}

impl Precompile for SetGasToken {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::SET_GAS_TOKEN_GAS)
    }

    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        context: &Context,
        is_static: bool,
    ) -> EvmPrecompileResult {
        let required_gas = Self::required_gas(input)?;
        if let Some(target_gas) = target_gas {
            if required_gas > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        // It's not allowed to call exit precompiles in static mode
        if is_static {
            return Err(ExitError::Other(Cow::from("ERR_INVALID_IN_STATIC")));
        } else if context.address != Self::ADDRESS.raw() {
            return Err(ExitError::Other(Cow::from("ERR_INVALID_IN_DELEGATE")));
        }

        let set_gas_token_log: Log = {
            let sender = Address::new(context.caller);
            let gas_token = Address::try_from_slice(input)
                .map_err(|_e| ExitError::Other(Cow::from("ERR_INVALID_ETH_ADDRESS")))?;
            SetGasTokenLog { sender, gas_token }.encode()
        };

        Ok(PrecompileOutput {
            cost: required_gas,
            logs: vec![set_gas_token_log],
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurora_engine_sdk::types::near_account_to_evm_address;

    #[test]
    fn test_precompile_id() {
        assert_eq!(
            SET_GAS_TOKEN_ADDRESS,
            near_account_to_evm_address("setGasToken".as_bytes())
        );
    }

    #[test]
    fn test_signature() {
        let schema = events::set_gas_token_schema();
        assert_eq!(schema.signature(), events::SET_GAS_TOKEN_SIGNATURE);
    }
}

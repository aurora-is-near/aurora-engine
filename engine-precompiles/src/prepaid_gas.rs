use super::{EvmPrecompileResult, Precompile};
use crate::prelude::types::{Address, EthGas};
use crate::PrecompileOutput;
use aurora_engine_sdk::env::Env;
use aurora_engine_types::{vec, U256};
use evm::{Context, ExitError};

/// prepaid_gas precompile address
///
/// Address: `0x536822d27de53629ef1f84c60555689e9488609f`
/// This address is computed as: `&keccak("prepaidGas")[12..]`
pub const ADDRESS: Address = crate::make_address(0x536822d2, 0x7de53629ef1f84c60555689e9488609f);

mod costs {
    use crate::prelude::types::EthGas;

    // TODO(#483): Determine the correct amount of gas
    pub(super) const PREPAID_GAS_COST: EthGas = EthGas::new(0);
}

pub struct PrepaidGas<'a, E> {
    env: &'a E,
}

impl<'a, E> PrepaidGas<'a, E> {
    pub fn new(env: &'a E) -> Self {
        Self { env }
    }
}

impl<'a, E: Env> Precompile for PrepaidGas<'a, E> {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::PREPAID_GAS_COST)
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

        let prepaid_gas = self.env.prepaid_gas();
        let bytes = {
            let mut buf = vec![0; 32];
            U256::from(prepaid_gas.as_u64()).to_big_endian(&mut buf);
            buf
        };
        Ok(PrecompileOutput::without_logs(cost, bytes))
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::sdk::types::near_account_to_evm_address;
    use crate::prepaid_gas;

    #[test]
    fn test_prepaid_gas_precompile_id() {
        assert_eq!(
            prepaid_gas::ADDRESS,
            near_account_to_evm_address("prepaidGas".as_bytes())
        );
    }
}

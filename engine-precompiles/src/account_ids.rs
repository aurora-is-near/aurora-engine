use super::{EvmPrecompileResult, Precompile};
use crate::prelude::types::{Address, EthGas};
use crate::PrecompileOutput;
use aurora_engine_sdk::env::Env;
use aurora_engine_types::account_id::AccountId;
use evm::{Context, ExitError};

mod costs {
    use crate::prelude::types::EthGas;

    // TODO(#483): Determine the correct amount of gas
    pub(super) const PREDECESSOR_ACCOUNT_GAS: EthGas = EthGas::new(0);
    // TODO(#483): Determine the correct amount of gas
    #[allow(dead_code)]
    pub(super) const CURRENT_ACCOUNT_GAS: EthGas = EthGas::new(0);
}

pub struct PredecessorAccount<'a, E> {
    env: &'a E,
}

pub mod predecessor_account {
    use aurora_engine_types::types::Address;

    /// `predecessor_account_id` precompile address
    ///
    /// Address: `0x723ffbaba940e75e7bf5f6d61dcbf8d9a4de0fd7`
    /// This address is computed as: `&keccak("predecessorAccountId")[12..]`
    pub const ADDRESS: Address =
        crate::make_address(0x723f_fbab, 0xa940_e75e_7bf5_f6d6_1dcb_f8d9_a4de_0fd7);
}

impl<'a, E> PredecessorAccount<'a, E> {
    pub const fn new(env: &'a E) -> Self {
        Self { env }
    }
}

impl<'a, E: Env> Precompile for PredecessorAccount<'a, E> {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::PREDECESSOR_ACCOUNT_GAS)
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

        let predecessor_account_id = self.env.predecessor_account_id();
        Ok(PrecompileOutput::without_logs(
            cost,
            predecessor_account_id.as_bytes().to_vec(),
        ))
    }
}

pub struct CurrentAccount {
    current_account_id: AccountId,
}

impl CurrentAccount {
    /// `current_account_id` precompile address
    ///
    /// Address: `0xfefae79e4180eb0284f261205e3f8cea737aff56`
    /// This address is computed as: `&keccak("currentAccountId")[12..]`
    pub const ADDRESS: Address =
        super::make_address(0xfefa_e79e, 0x4180_eb02_84f2_6120_5e3f_8cea_737a_ff56);

    #[must_use]
    pub const fn new(current_account_id: AccountId) -> Self {
        Self { current_account_id }
    }
}

impl Precompile for CurrentAccount {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::PREDECESSOR_ACCOUNT_GAS)
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

        Ok(PrecompileOutput::without_logs(
            cost,
            self.current_account_id.as_bytes().to_vec(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::account_ids::{predecessor_account, CurrentAccount};
    use crate::prelude::sdk::types::near_account_to_evm_address;

    #[test]
    fn test_predecessor_account_precompile_id() {
        assert_eq!(
            predecessor_account::ADDRESS,
            near_account_to_evm_address(b"predecessorAccountId")
        );
    }

    #[test]
    fn test_curent_account_precompile_id() {
        assert_eq!(
            CurrentAccount::ADDRESS,
            near_account_to_evm_address(b"currentAccountId")
        );
    }
}

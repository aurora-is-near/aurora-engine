use super::{EvmPrecompileResult, Precompile};
use crate::prelude::types::EthGas;
use crate::prelude::{Address, Vec};
use crate::PrecompileOutput;
use evm::{Context, ExitError};

mod costs {
    use crate::prelude::types::EthGas;

    // TODO(#51): Determine the correct amount of gas
    pub(super) const RANDOM_BYTES_GAS: EthGas = EthGas::new(0);
}

pub struct RandomSeed {
    random_seed: Vec<u8>,
}

impl RandomSeed {
    /// Random bytes precompile address
    /// This is a per-block entropy source which could then be used to create a random sequence.
    /// It will return the same seed if called multiple time in the same block.
    ///
    /// Address: `0xc104f4840573bed437190daf5d2898c2bdf928ac`
    /// This address is computed as: `&keccak("randomSeed")[12..]`
    pub const ADDRESS: Address =
        super::make_address(0xc104f484, 0x0573bed437190daf5d2898c2bdf928ac);

    pub fn new(random_seed: Vec<u8>) -> Self {
        Self { random_seed }
    }
}

impl Precompile for RandomSeed {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError> {
        Ok(costs::RANDOM_BYTES_GAS)
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

        Ok(PrecompileOutput::without_logs(cost, self.random_seed.clone()).into())
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::sdk::types::near_account_to_evm_address;
    use crate::random::RandomSeed;

    #[test]
    fn test_precompile_id() {
        assert_eq!(
            RandomSeed::ADDRESS,
            near_account_to_evm_address("randomSeed".as_bytes())
        );
    }
}

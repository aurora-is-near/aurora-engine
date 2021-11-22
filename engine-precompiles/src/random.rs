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

const ERR_TARGET_TOKEN_NOT_FOUND: &str = "Target token not found";

pub struct RandomBytes {
    random_seed: Vec<u8>,
}

impl RandomBytes {
    /// Random bytes precompile address
    ///
    /// Address: `0xf861511815955326b953fa97b6955a2f8020a4e9`
    /// This address is computed as: `&keccak("randomBytes")[12..]`
    pub const ADDRESS: Address =
        super::make_address(0xf8615118, 0x15955326b953fa97b6955a2f8020a4e9);

    pub fn new(random_seed: Vec<u8>) -> Self {
        Self { random_seed }
    }
}

impl Precompile for RandomBytes {
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
    use crate::random::RandomBytes;

    #[test]
    fn test_precompile_id() {
        assert_eq!(
            RandomBytes::ADDRESS,
            near_account_to_evm_address("randomBytes".as_bytes())
        );
    }
}

#[cfg(feature = "contract")]
use crate::prelude::sdk;
use crate::prelude::types::{Address, EthGas};
use crate::prelude::vec;
use crate::{utils, EvmPrecompileResult, Precompile, PrecompileOutput};
use evm::{Context, ExitError};

mod costs {
    use crate::prelude::types::EthGas;

    pub(super) const SHA256_BASE: EthGas = EthGas::new(60);

    pub(super) const SHA256_PER_WORD: EthGas = EthGas::new(12);

    pub(super) const RIPEMD160_BASE: EthGas = EthGas::new(600);

    pub(super) const RIPEMD160_PER_WORD: EthGas = EthGas::new(120);
}

mod consts {
    pub(super) const SHA256_WORD_LEN: u64 = 32;

    pub(super) const RIPEMD_WORD_LEN: u64 = 32;
}

/// SHA256 precompile.
pub struct SHA256;

impl SHA256 {
    pub const ADDRESS: Address = super::make_address(0, 2);
}

impl Precompile for SHA256 {
    fn required_gas(input: &[u8]) -> Result<EthGas, ExitError> {
        let input_len = u64::try_from(input.len()).map_err(utils::err_usize_conv)?;
        Ok(
            (input_len + consts::SHA256_WORD_LEN - 1) / consts::SHA256_WORD_LEN
                * costs::SHA256_PER_WORD
                + costs::SHA256_BASE,
        )
    }

    /// See: https://ethereum.github.io/yellowpaper/paper.pdf
    /// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
    /// See: https://etherscan.io/address/0000000000000000000000000000000000000002
    #[cfg(not(feature = "contract"))]
    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        _context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        use sha2::Digest;

        let cost = Self::required_gas(input)?;
        if let Some(target_gas) = target_gas {
            if cost > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        let output = sha2::Sha256::digest(input).to_vec();
        Ok(PrecompileOutput::without_logs(cost, output))
    }

    /// See: https://ethereum.github.io/yellowpaper/paper.pdf
    /// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
    /// See: https://etherscan.io/address/0000000000000000000000000000000000000002
    #[cfg(feature = "contract")]
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

        let output = sdk::sha256(input).as_bytes().to_vec();
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

/// RIPEMD160 precompile.
pub struct RIPEMD160;

impl RIPEMD160 {
    pub const ADDRESS: Address = super::make_address(0, 3);

    #[cfg(not(feature = "contract"))]
    fn internal_impl(input: &[u8]) -> [u8; 20] {
        use ripemd::{Digest, Ripemd160};

        let hash = Ripemd160::digest(input);
        let mut output = [0u8; 20];
        output.copy_from_slice(&hash);
        output
    }
}

impl Precompile for RIPEMD160 {
    fn required_gas(input: &[u8]) -> Result<EthGas, ExitError> {
        let input_len = u64::try_from(input.len()).map_err(utils::err_usize_conv)?;
        Ok(
            (input_len + consts::RIPEMD_WORD_LEN - 1) / consts::RIPEMD_WORD_LEN
                * costs::RIPEMD160_PER_WORD
                + costs::RIPEMD160_BASE,
        )
    }

    /// See: https://ethereum.github.io/yellowpaper/paper.pdf
    /// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
    /// See: https://etherscan.io/address/0000000000000000000000000000000000000003
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

        #[cfg(not(feature = "contract"))]
        let hash = Self::internal_impl(input);
        #[cfg(feature = "contract")]
        let hash = sdk::ripemd160(input);
        // The result needs to be padded with leading zeros because it is only 20 bytes, but
        // the evm works with 32-byte words.
        let mut output = vec![0u8; 32];
        output[12..].copy_from_slice(&hash);
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::new_context;

    use super::*;

    #[test]
    fn test_sha256() {
        let input = b"";
        let expected =
            hex::decode("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
                .unwrap();

        let res = SHA256
            .run(input, Some(EthGas::new(60)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);
    }

    #[test]
    fn test_ripemd160() {
        let input = b"";
        let expected =
            hex::decode("0000000000000000000000009c1185a5c5e9fc54612808977ee8f548b2258d31")
                .unwrap();

        let res = RIPEMD160
            .run(input, Some(EthGas::new(600)), &new_context(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);
    }
}

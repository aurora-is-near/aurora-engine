use crate::precompiles::{Precompile, PrecompileResult};
use evm::{Context, ExitError, ExitSucceed};

mod costs {
    pub(super) const SHA256_BASE: u64 = 60;

    pub(super) const SHA256_PER_WORD: u64 = 12;

    pub(super) const RIPEMD160_BASE: u64 = 600;

    pub(super) const RIPEMD160_PER_WORD: u64 = 12;
}

mod consts {
    pub(super) const SHA256_WORD_LEN: u64 = 32;

    pub(super) const RIPEMD_WORD_LEN: u64 = 32;
}

/// SHA256 precompile.
pub struct SHA256;

impl Precompile for SHA256 {
    fn required_gas(input: &[u8]) -> Result<u64, ExitError> {
        Ok(
            (input.len() as u64 + consts::SHA256_WORD_LEN - 1) / consts::SHA256_WORD_LEN
                * costs::SHA256_PER_WORD
                + costs::SHA256_BASE,
        )
    }

    /// See: https://ethereum.github.io/yellowpaper/paper.pdf
    /// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
    /// See: https://etherscan.io/address/0000000000000000000000000000000000000002
    #[cfg(not(feature = "contract"))]
    fn run(input: &[u8], target_gas: u64, _context: &Context) -> PrecompileResult {
        use sha2::Digest;

        if Self::required_gas(input)? > target_gas {
            return Err(ExitError::OutOfGas);
        }

        let hash = sha2::Sha256::digest(input);
        Ok((ExitSucceed::Returned, hash.to_vec(), 0))
    }

    /// See: https://ethereum.github.io/yellowpaper/paper.pdf
    /// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
    /// See: https://etherscan.io/address/0000000000000000000000000000000000000002
    #[cfg(feature = "contract")]
    fn run(input: &[u8], target_gas: u64, _context: &Context) -> PrecompileResult {
        use crate::sdk;

        if Self::required_gas(input)? > target_gas {
            Err(ExitError::OutOfGas)
        } else {
            Ok((
                ExitSucceed::Returned,
                sdk::sha256(input).as_bytes().to_vec(),
                0,
            ))
        }
    }
}

/// RIPEMD160 precompile.
pub struct RIPEMD160;

impl Precompile for RIPEMD160 {
    fn required_gas(input: &[u8]) -> Result<u64, ExitError> {
        Ok(
            (input.len() as u64 + consts::RIPEMD_WORD_LEN - 1) / consts::RIPEMD_WORD_LEN
                * costs::RIPEMD160_PER_WORD
                + costs::RIPEMD160_BASE,
        )
    }

    /// See: https://ethereum.github.io/yellowpaper/paper.pdf
    /// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
    /// See: https://etherscan.io/address/0000000000000000000000000000000000000003
    fn run(input: &[u8], target_gas: u64, _context: &Context) -> PrecompileResult {
        use ripemd160::Digest;

        if Self::required_gas(input)? > target_gas {
            Err(ExitError::OutOfGas)
        } else {
            let hash = ripemd160::Ripemd160::digest(input);
            Ok((ExitSucceed::Returned, hash.to_vec(), 0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_context() -> Context {
        Context {
            address: Default::default(),
            caller: Default::default(),
            apparent_value: Default::default(),
        }
    }

    #[test]
    fn test_sha256() {
        let input = b"";
        let expected =
            hex::decode("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
                .unwrap();

        let res = SHA256::run(input, 60, &new_context()).unwrap().1;
        assert_eq!(res, expected);
    }

    #[test]
    fn test_ripemd160() {
        let input = b"";
        let expected = hex::decode("9c1185a5c5e9fc54612808977ee8f548b2258d31").unwrap();

        let res = RIPEMD160::run(input, 600, &new_context()).unwrap().1;
        assert_eq!(res, expected);
    }
}

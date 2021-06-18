use crate::precompiles::{Precompile, PrecompileOutput, PrecompileResult};
use crate::prelude::{vec, PhantomData};
use evm::{Context, ExitError};

use crate::AuroraState;

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
pub struct SHA256<S>(PhantomData<S>);

impl<S> SHA256<S> {
    pub(super) const ADDRESS: [u8; 20] = super::make_address(0, 2);
}

impl<S: AuroraState> Precompile<S> for SHA256<S> {
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
    fn run(
        input: &[u8],
        target_gas: u64,
        _context: &Context,
        _state: &mut S,
        _is_static: bool,
    ) -> PrecompileResult {
        use sha2::Digest;

        if Self::required_gas(input)? > target_gas {
            return Err(ExitError::OutOfGas);
        }

        let cost = Self::required_gas(input)?;
        if cost > target_gas {
            Err(ExitError::OutOfGas)
        } else {
            let output = sha2::Sha256::digest(input).to_vec();
            Ok(PrecompileOutput::without_logs(cost, output))
        }
    }

    /// See: https://ethereum.github.io/yellowpaper/paper.pdf
    /// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
    /// See: https://etherscan.io/address/0000000000000000000000000000000000000002
    #[cfg(feature = "contract")]
    fn run(
        input: &[u8],
        target_gas: u64,
        _context: &Context,
        _state: &mut S,
        _is_static: bool,
    ) -> PrecompileResult {
        use crate::sdk;

        let cost = Self::required_gas(input)?;
        if cost > target_gas {
            Err(ExitError::OutOfGas)
        } else {
            let output = sdk::sha256(input).as_bytes().to_vec();
            Ok(PrecompileOutput::without_logs(cost, output))
        }
    }
}

/// RIPEMD160 precompile.
pub struct RIPEMD160<S>(PhantomData<S>);

impl<S> RIPEMD160<S> {
    pub(super) const ADDRESS: [u8; 20] = super::make_address(0, 3);
}

impl<S: AuroraState> Precompile<S> for RIPEMD160<S> {
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
    fn run(
        input: &[u8],
        target_gas: u64,
        _context: &Context,
        _state: &mut S,
        _is_static: bool,
    ) -> PrecompileResult {
        use ripemd160::Digest;

        let cost = Self::required_gas(input)?;
        if cost > target_gas {
            Err(ExitError::OutOfGas)
        } else {
            let hash = ripemd160::Ripemd160::digest(input);
            // The result needs to be padded with leading zeros because it is only 20 bytes, but
            // the evm works with 32-byte words.
            let mut output = vec![0u8; 32];
            output[12..].copy_from_slice(&hash);
            Ok(PrecompileOutput::without_logs(cost, output))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::{new_context, new_state};

    use super::*;

    #[test]
    fn test_sha256() {
        let input = b"";
        let expected =
            hex::decode("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
                .unwrap();

        let res = SHA256::run(input, 60, &new_context(), &mut new_state(), false)
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

        let res = RIPEMD160::run(input, 600, &new_context(), &mut new_state(), false)
            .unwrap()
            .output;
        assert_eq!(res, expected);
    }
}

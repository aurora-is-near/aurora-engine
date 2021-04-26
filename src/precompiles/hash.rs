use crate::precompiles::PrecompileResult;
use evm::ExitSucceed;

mod costs {
    pub(super) const SHA256_BASE: u64 = 60;

    pub(super) const SHA256_PER_WORD: u64 = 12;

    pub(super) const RIPEMD160_BASE: u64 = 600;

    pub(super) const RIPEMD160_PER_WORD: u64 = 12;
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
/// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
/// See: https://etherscan.io/address/0000000000000000000000000000000000000002
#[cfg(not(feature = "contract"))]
pub(crate) fn sha256(input: &[u8], target_gas: Option<u64>) -> PrecompileResult {
    use sha2::Digest;

    let cost = (input.len() + 31) as u64 / 32 * costs::SHA256_PER_WORD + costs::SHA256_BASE;
    super::util::check_gas(target_gas, cost)?;

    let hash = sha2::Sha256::digest(input);
    Ok((ExitSucceed::Returned, hash.to_vec(), 0))
}

#[cfg(feature = "contract")]
pub(crate) fn sha256(input: &[u8], target_gas: Option<u64>) -> PrecompileResult {
    use crate::sdk;

    let cost = (input.len() + 31) as u64 / 32 * costs::SHA256_PER_WORD + costs::SHA256_BASE;
    super::check_gas(target_gas, cost)?;

    Ok((
        ExitSucceed::Returned,
        sdk::sha256(input).as_bytes().to_vec(),
        0,
    ))
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
/// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
/// See: https://etherscan.io/address/0000000000000000000000000000000000000003
pub(crate) fn ripemd160(input: &[u8], target_gas: Option<u64>) -> PrecompileResult {
    use ripemd160::Digest;

    let cost = (input.len() + 31) as u64 / 32 * costs::RIPEMD160_PER_WORD + costs::RIPEMD160_BASE;
    super::check_gas(target_gas, cost)?;

    let hash = ripemd160::Ripemd160::digest(input);
    Ok((ExitSucceed::Returned, hash.to_vec(), 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256() {
        let input = b"";
        let expected =
            hex::decode("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
                .unwrap();

        let res = sha256(input, Some(60)).unwrap().1;
        assert_eq!(res, expected);
    }

    #[test]
    fn test_ripemd160() {
        let input = b"";
        let expected = hex::decode("9c1185a5c5e9fc54612808977ee8f548b2258d31").unwrap();

        let res = ripemd160(input, Some(600)).unwrap().1;
        assert_eq!(res, expected);
    }
}

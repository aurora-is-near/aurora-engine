use crate::prelude::{H256, H160};

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
/// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
/// See: https://etherscan.io/address/0000000000000000000000000000000000000002
#[cfg(not(feature = "contract"))]
pub(crate) fn sha256(input: &[u8]) -> H256 {
    use sha2::Digest;
    let hash = sha2::Sha256::digest(input);
    H256::from_slice(&hash)
}

#[cfg(feature = "contract")]
pub(crate) fn sha256(input: &[u8]) -> H256 {
    use crate::sdk;
    sdk::sha256(input)
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
/// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
/// See: https://etherscan.io/address/0000000000000000000000000000000000000003
pub(crate) fn ripemd160(input: &[u8]) -> H160 {
    use ripemd160::Digest;
    let hash = ripemd160::Ripemd160::digest(input);
    H160::from_slice(&hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256() {
        assert_eq!(
            sha256(b""),
            H256::from_slice(
                &hex::decode("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_ripemd160() {
        assert_eq!(
            ripemd160(b""),
            H160::from_slice(&hex::decode("9c1185a5c5e9fc54612808977ee8f548b2258d31").unwrap())
        );
    }
}

use crate::prelude::{Address, Vec, H160, H256, U256};
use evm::{Context, ExitError, ExitSucceed};

type PrecompileResult = Result<(ExitSucceed, Vec<u8>, u64), ExitError>;

#[allow(dead_code)]
pub fn no_precompiles(
    _address: Address,
    _input: &[u8],
    _target_gas: Option<u64>,
    _context: &Context,
) -> Option<PrecompileResult> {
    None // not precompiles supported
}

#[allow(dead_code)]
pub fn istanbul_precompiles(
    address: Address,
    input: &[u8],
    _target_gas: Option<u64>,
    _context: &Context,
) -> Option<PrecompileResult> {
    match address.to_low_u64_be() {
        1 => todo!(), // TODO: implement ecrecover(),
        2 => Some(Ok((
            ExitSucceed::Returned,
            sha256(input).as_bytes().to_vec(),
            0,
        ))),
        3 => Some(Ok((
            ExitSucceed::Returned,
            ripemd160(input).as_bytes().to_vec(),
            0,
        ))),
        4 => Some(Ok((ExitSucceed::Returned, identity(input).to_vec(), 0))),
        5 => todo!(), // TODO: implement modexp()
        6 => todo!(), // TODO: implement alt_bn128_add()
        7 => todo!(), // TODO: implement alt_bn128_mul()
        8 => todo!(), // TODO: implement alt_bn128_pair()
        9 => todo!(), // TODO: implement blake2f()
        // Not supported.
        _ => None,
    }
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
#[allow(dead_code)]
fn ecrecover(_hash: H256, _v: u8, _r: H256, _s: H256) -> Address {
    Address::zero() // TODO: implement ECRECOVER
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
/// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000002
#[cfg(not(feature = "contract"))]
fn sha256(input: &[u8]) -> H256 {
    use sha2::Digest;
    let hash = sha2::Sha256::digest(input);
    H256::from_slice(&hash)
}
#[cfg(feature = "contract")]
fn sha256(input: &[u8]) -> H256 {
    use crate::sdk;
    sdk::sha256(input)
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
/// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000003
fn ripemd160(input: &[u8]) -> H160 {
    use ripemd160::Digest;
    let hash = ripemd160::Ripemd160::digest(input);
    H160::from_slice(&hash)
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
fn identity(input: &[u8]) -> &[u8] {
    input
}

/// See: https://eips.ethereum.org/EIPS/eip-198
#[allow(dead_code)]
fn modexp(_base: U256, _exponent: U256, _modulus: U256) -> U256 {
    U256::zero() // TODO: implement MODEXP
}

/// See: https://eips.ethereum.org/EIPS/eip-196
#[allow(dead_code)]
fn alt_bn128_add(_ax: U256, _ay: U256, _bx: U256, _by: U256) {
    // TODO: implement alt_bn128_add
}

/// See: https://eips.ethereum.org/EIPS/eip-196
#[allow(dead_code)]
fn alt_bn128_mul(_x: U256, _y: U256, _scalar: U256) {
    // TODO: implement alt_bn128_mul
}

/// See: https://eips.ethereum.org/EIPS/eip-197
#[allow(dead_code)]
fn alt_bn128_pair(_input: Vec<u8>) -> U256 {
    U256::zero() // TODO: implement alt_bn128_pairing
}

/// See: https://eips.ethereum.org/EIPS/eip-152
#[allow(dead_code)]
fn blake2f(_rounds: u32, _h: [U256; 2], _m: [U256; 4], _t: [u64; 2], _f: bool) -> [U256; 2] {
    [U256::zero(), U256::zero()] // TODO: implement BLAKE2f
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

    #[test]
    fn test_identity() {
        assert_eq!(identity(b""), b"")
    }
}

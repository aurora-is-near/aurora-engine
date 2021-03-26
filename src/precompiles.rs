use crate::prelude::{Address, Borrowed, Vec, H160, H256, U256};
use evm::{Context, ExitError, ExitSucceed};

type PrecompileResult = Result<(ExitSucceed, Vec<u8>, u64), ExitError>;

#[allow(dead_code)]
pub fn no_precompiles(
    _address: Address,
    _input: &[u8],
    _target_gas: Option<u64>,
    _context: &Context,
) -> Option<PrecompileResult> {
    None // no precompiles supported
}

#[allow(dead_code)]
pub fn istanbul_precompiles(
    address: Address,
    input: &[u8],
    _target_gas: Option<u64>,
    _context: &Context,
) -> Option<PrecompileResult> {
    match address.to_low_u64_be() {
        1 => Some(Ok((
            ExitSucceed::Returned,
            ecrecover_raw(input).as_bytes().to_vec(),
            0,
        ))),
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

#[allow(dead_code)]
fn ecrecover_raw(input: &[u8]) -> Address {
    assert_eq!(input.len(), 128); // input is (hash, v, r, s), each typed as a uint256

    let mut hash = [0; 32];
    hash.copy_from_slice(&input[0..32]);

    let mut signature = [0; 65];  // signature is (r, s, v), typed (uint256, uint256, uint8)
    signature[0..32].copy_from_slice(&input[64..]);  // r
    signature[32..64].copy_from_slice(&input[96..]); // s
    signature[64] = input[63];                       // v

    ecrecover(H256::from_slice(&hash), &signature).unwrap_or_else(|_| Address::zero())
}

#[allow(dead_code)]
pub(crate) fn ecverify(hash: H256, signature: &[u8], signer: Address) -> bool {
    match ecrecover(hash, signature) {
        Ok(s) if s == signer => true,
        _ => false,
    }
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
/// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000001
#[allow(dead_code)]
pub(crate) fn ecrecover(hash: H256, signature: &[u8]) -> Result<Address, ExitError> {
    use sha3::Digest;
    assert_eq!(signature.len(), 65);

    let hash = secp256k1::Message::parse_slice(hash.as_bytes()).unwrap();
    let v = signature[64];
    let signature = secp256k1::Signature::parse_slice(&signature[0..64]).unwrap();
    let bit = match v {
        0..=26 => v,
        _ => v - 27,
    };

    if let Ok(recovery_id) = secp256k1::RecoveryId::parse(bit) {
        if let Ok(public_key) = secp256k1::recover(&hash, &signature, &recovery_id) {
            // recover returns a 65-byte key, but addresses come from the raw 64-byte key
            let r = sha3::Keccak256::digest(&public_key.serialize()[1..]);
            return Ok(Address::from_slice(&r[12..]));
        }
    }
    Err(ExitError::Other(Borrowed("invalid ECDSA signature")))
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
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000004
fn identity(input: &[u8]) -> &[u8] {
    input
}

/// See: https://eips.ethereum.org/EIPS/eip-198
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000005
#[allow(dead_code)]
fn modexp(_base: U256, _exponent: U256, _modulus: U256) -> U256 {
    U256::zero() // TODO: implement MODEXP
}

/// See: https://eips.ethereum.org/EIPS/eip-196
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000006
#[allow(dead_code)]
fn alt_bn128_add(_ax: U256, _ay: U256, _bx: U256, _by: U256) {
    // TODO: implement alt_bn128_add
}

/// See: https://eips.ethereum.org/EIPS/eip-196
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000007
#[allow(dead_code)]
fn alt_bn128_mul(_x: U256, _y: U256, _scalar: U256) {
    // TODO: implement alt_bn128_mul
}

/// See: https://eips.ethereum.org/EIPS/eip-197
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000008
#[allow(dead_code)]
fn alt_bn128_pair(_input: Vec<u8>) -> U256 {
    U256::zero() // TODO: implement alt_bn128_pairing
}

/// See: https://eips.ethereum.org/EIPS/eip-152
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000009
#[allow(dead_code)]
fn blake2f(_rounds: u32, _h: [U256; 2], _m: [U256; 4], _t: [u64; 2], _f: bool) -> [U256; 2] {
    [U256::zero(), U256::zero()] // TODO: implement BLAKE2f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ecverify() {
        let hash = H256::from_slice(
            &hex::decode("1111111111111111111111111111111111111111111111111111111111111111")
                .unwrap(),
        );
        let signature =
                &hex::decode("b9f0bb08640d3c1c00761cdd0121209268f6fd3816bc98b9e6f3cc77bf82b69812ac7a61788a0fdc0e19180f14c945a8e1088a27d92a74dce81c0981fb6447441b")
                    .unwrap();
        let signer =
            Address::from_slice(&hex::decode("1563915e194D8CfBA1943570603F7606A3115508").unwrap());
        assert!(ecverify(hash, &signature, signer));
    }

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

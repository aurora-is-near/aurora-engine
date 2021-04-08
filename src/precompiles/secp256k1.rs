use ethabi::Address;
use evm::ExitError;
use crate::prelude::{Borrowed, H256};

pub(crate) fn ecrecover_raw(input: &[u8]) -> Address {
    assert_eq!(input.len(), 128); // input is (hash, v, r, s), each typed as a uint256

    let mut hash = [0; 32];
    hash.copy_from_slice(&input[0..32]);

    let mut signature = [0; 65]; // signature is (r, s, v), typed (uint256, uint256, uint8)
    signature[0..32].copy_from_slice(&input[64..]); // r
    signature[32..64].copy_from_slice(&input[96..]); // s
    signature[64] = input[63]; // v

    ecrecover(H256::from_slice(&hash), &signature).unwrap_or_else(|_| Address::zero())
}

#[allow(dead_code)]
pub(crate) fn ecverify(hash: H256, signature: &[u8], signer: Address) -> bool {
    matches!(ecrecover(hash, signature), Ok(s) if s == signer)
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
/// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
/// See: https://etherscan.io/address/0000000000000000000000000000000000000001
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
}

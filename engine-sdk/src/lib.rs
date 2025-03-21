#![cfg_attr(not(feature = "std"), no_std)]
// All `as` conversions in this code base have been carefully reviewed and are safe.
#![allow(clippy::as_conversions)]

use crate::prelude::{Address, H256, STORAGE_PRICE_PER_BYTE};
#[cfg(feature = "contract")]
use crate::prelude::{Vec, U256};
pub use types::keccak;

pub mod base64;
#[cfg(feature = "std")]
pub mod bls12_381;
pub mod caching;
pub mod env;
pub mod error;
pub mod io;
#[cfg(feature = "contract")]
pub mod near_runtime;
mod prelude;
pub mod promise;
pub mod types;

#[cfg(feature = "contract")]
use near_runtime::exports;

#[cfg(feature = "contract")]
const ECRECOVER_MESSAGE_SIZE: u64 = 32;
#[cfg(feature = "contract")]
const ECRECOVER_SIGNATURE_LENGTH: u64 = 64;
#[cfg(feature = "contract")]
const ECRECOVER_MALLEABILITY_FLAG: u64 = 0;

#[cfg(feature = "contract")]
pub fn panic_utf8(bytes: &[u8]) -> ! {
    unsafe {
        exports::panic_utf8(bytes.len() as u64, bytes.as_ptr() as u64);
    }
    unreachable!()
}

#[cfg(feature = "contract")]
pub fn log_utf8(bytes: &[u8]) {
    unsafe {
        exports::log_utf8(bytes.len() as u64, bytes.as_ptr() as u64);
    }
}

/// Calls environment sha256 on given input.
#[cfg(feature = "contract")]
#[must_use]
pub fn sha256(input: &[u8]) -> H256 {
    unsafe {
        const REGISTER_ID: u64 = 1;
        exports::sha256(input.len() as u64, input.as_ptr() as u64, 1);
        let bytes = H256::zero();
        exports::read_register(REGISTER_ID, bytes.0.as_ptr() as u64);
        bytes
    }
}

#[cfg(not(feature = "contract"))]
#[must_use]
pub fn sha256(input: &[u8]) -> H256 {
    use sha2::Digest;

    let output = sha2::Sha256::digest(input);
    H256(output.into())
}

/// Calls environment ripemd160 on given input.
#[cfg(feature = "contract")]
#[must_use]
pub fn ripemd160(input: &[u8]) -> [u8; 20] {
    unsafe {
        const REGISTER_ID: u64 = 1;
        exports::ripemd160(input.len() as u64, input.as_ptr() as u64, REGISTER_ID);
        let bytes = [0u8; 20];
        exports::read_register(REGISTER_ID, bytes.as_ptr() as u64);
        bytes
    }
}

#[cfg(feature = "contract")]
#[must_use]
pub fn alt_bn128_g1_sum(left: [u8; 64], right: [u8; 64]) -> [u8; 64] {
    let mut bytes = Vec::with_capacity(64 * 2 + 2); // 64 bytes per G1 + 2 positive integer bytes.

    bytes.push(0); // positive sign
    bytes.extend_from_slice(&left);
    bytes.push(0);
    bytes.extend_from_slice(&right);

    let value_ptr = bytes.as_ptr() as u64;
    let value_len = bytes.len() as u64;

    unsafe {
        const REGISTER_ID: u64 = 1;
        exports::alt_bn128_g1_sum(value_len, value_ptr, REGISTER_ID);
        let mut output = [0u8; 64];
        exports::read_register(REGISTER_ID, output.as_ptr() as u64);
        let x = U256::from_little_endian(&output[0..32]);
        let y = U256::from_little_endian(&output[32..64]);
        output[0..32].copy_from_slice(&x.to_big_endian());
        output[32..64].copy_from_slice(&y.to_big_endian());
        output
    }
}

#[cfg(feature = "contract")]
#[must_use]
pub fn alt_bn128_g1_scalar_multiple(g1: [u8; 64], fr: [u8; 32]) -> [u8; 64] {
    let mut bytes = [0u8; 96];
    bytes[0..64].copy_from_slice(&g1);
    bytes[64..96].copy_from_slice(&fr);

    let value_ptr = bytes.as_ptr() as u64;
    let value_len = bytes.len() as u64;

    unsafe {
        const REGISTER_ID: u64 = 1;
        exports::alt_bn128_g1_multiexp(value_len, value_ptr, REGISTER_ID);
        let mut output = [0u8; 64];
        exports::read_register(REGISTER_ID, output.as_ptr() as u64);
        let x = U256::from_little_endian(&output[0..32]);
        let y = U256::from_little_endian(&output[32..64]);
        output[0..32].copy_from_slice(&x.to_big_endian());
        output[32..64].copy_from_slice(&y.to_big_endian());
        output
    }
}

#[cfg(feature = "contract")]
pub fn alt_bn128_pairing<I>(pairs: I) -> bool
where
    I: ExactSizeIterator<Item = ([u8; 64], [u8; 128])>,
{
    let n = pairs.len();
    let mut bytes = Vec::with_capacity(n * 6 * 32);
    let mut buf = [0u8; 64 + 128];
    for (g1, g2) in pairs {
        buf[0..64].copy_from_slice(&g1);
        buf[64..192].copy_from_slice(&g2);
        bytes.extend_from_slice(&buf);
    }

    let value_ptr = bytes.as_ptr() as u64;
    let value_len = bytes.len() as u64;

    let result = unsafe { exports::alt_bn128_pairing_check(value_len, value_ptr) };

    result == 1
}

#[cfg(feature = "contract")]
#[must_use]
pub fn bls12381_p1_sum(input: &[u8]) -> [u8; 96] {
    unsafe {
        const REGISTER_ID: u64 = 1;
        exports::bls12381_p1_sum(input.len() as u64, input.as_ptr() as u64, REGISTER_ID);
        let bytes = [0u8; 96];
        exports::read_register(REGISTER_ID, bytes.as_ptr() as u64);
        bytes
    }
}

#[cfg(feature = "contract")]
#[must_use]
pub fn bls12381_p2_sum(input: &[u8]) -> [u8; 192] {
    unsafe {
        const REGISTER_ID: u64 = 1;
        exports::bls12381_p2_sum(input.len() as u64, input.as_ptr() as u64, REGISTER_ID);
        let bytes = [0u8; 192];
        exports::read_register(REGISTER_ID, bytes.as_ptr() as u64);
        bytes
    }
}

#[cfg(feature = "contract")]
#[must_use]
pub fn bls12381_g1_multiexp(input: &[u8]) -> [u8; 96] {
    unsafe {
        const REGISTER_ID: u64 = 1;
        exports::bls12381_g1_multiexp(input.len() as u64, input.as_ptr() as u64, REGISTER_ID);
        let bytes = [0u8; 96];
        exports::read_register(REGISTER_ID, bytes.as_ptr() as u64);
        bytes
    }
}

#[cfg(feature = "contract")]
#[must_use]
pub fn bls12381_g2_multiexp(input: &[u8]) -> [u8; 192] {
    unsafe {
        const REGISTER_ID: u64 = 1;
        exports::bls12381_g2_multiexp(input.len() as u64, input.as_ptr() as u64, REGISTER_ID);
        let bytes = [0u8; 192];
        exports::read_register(REGISTER_ID, bytes.as_ptr() as u64);
        bytes
    }
}

#[cfg(feature = "contract")]
#[must_use]
pub fn bls12381_map_fp_to_g1(input: &[u8]) -> [u8; 96] {
    unsafe {
        const REGISTER_ID: u64 = 1;
        exports::bls12381_map_fp_to_g1(input.len() as u64, input.as_ptr() as u64, REGISTER_ID);
        let bytes = [0u8; 96];
        exports::read_register(REGISTER_ID, bytes.as_ptr() as u64);
        bytes
    }
}

#[cfg(feature = "contract")]
#[must_use]
pub fn bls12381_map_fp2_to_g2(input: &[u8]) -> [u8; 192] {
    unsafe {
        const REGISTER_ID: u64 = 1;
        exports::bls12381_map_fp2_to_g2(input.len() as u64, input.as_ptr() as u64, REGISTER_ID);
        let bytes = [0u8; 192];
        exports::read_register(REGISTER_ID, bytes.as_ptr() as u64);
        bytes
    }
}

#[cfg(feature = "contract")]
#[must_use]
pub fn bls12381_pairing_check(input: &[u8]) -> u64 {
    unsafe { exports::bls12381_pairing_check(input.len() as u64, input.as_ptr() as u64) }
}

/// Recover address from message hash and signature.
#[cfg(feature = "contract")]
pub fn ecrecover(hash: H256, signature: &[u8]) -> Result<Address, ECRecoverErr> {
    unsafe {
        const RECOVER_REGISTER_ID: u64 = 1;
        const KECCACK_REGISTER_ID: u64 = 2;

        let hash_ptr = hash.as_ptr() as u64;
        let sig_ptr = signature.as_ptr() as u64;
        let result = exports::ecrecover(
            ECRECOVER_MESSAGE_SIZE,
            hash_ptr,
            ECRECOVER_SIGNATURE_LENGTH,
            sig_ptr,
            u64::from(signature[64]),
            ECRECOVER_MALLEABILITY_FLAG,
            RECOVER_REGISTER_ID,
        );
        if result == u64::from(true) {
            // The result from the ecrecover call is in a register; we can use this
            // register directly for the input to keccak256. This is why the length is
            // set to `u64::MAX`.
            exports::keccak256(u64::MAX, RECOVER_REGISTER_ID, KECCACK_REGISTER_ID);
            let keccak_hash_bytes = [0u8; 32];
            exports::read_register(KECCACK_REGISTER_ID, keccak_hash_bytes.as_ptr() as u64);
            Ok(Address::try_from_slice(&keccak_hash_bytes[12..]).map_err(|_| ECRecoverErr)?)
        } else {
            Err(ECRecoverErr)
        }
    }
}

#[cfg(not(feature = "contract"))]
pub fn ecrecover(hash: H256, signature: &[u8]) -> Result<Address, ECRecoverErr> {
    use sha3::Digest;

    let hash = libsecp256k1::Message::parse_slice(hash.as_bytes()).map_err(|_| ECRecoverErr)?;
    let v = signature[64];
    let signature = libsecp256k1::Signature::parse_standard_slice(&signature[0..64])
        .map_err(|_| ECRecoverErr)?;
    let bit = match v {
        0..=26 => v,
        _ => v - 27,
    };
    let recovery_id = libsecp256k1::RecoveryId::parse(bit).map_err(|_| ECRecoverErr)?;

    libsecp256k1::recover(&hash, &signature, &recovery_id)
        .map_err(|_| ECRecoverErr)
        .and_then(|public_key| {
            // recover returns a 65-byte key, but addresses come from the raw 64-byte key
            let r = sha3::Keccak256::digest(&public_key.serialize()[1..]);
            Address::try_from_slice(&r[12..]).map_err(|_| ECRecoverErr)
        })
}

#[cfg(feature = "contract")]
pub fn log(data: &str) {
    log_utf8(data.as_bytes());
}

#[cfg(not(feature = "contract"))]
#[allow(clippy::missing_const_for_fn)]
pub fn log(_data: &str) {
    // TODO: standalone logging
}

#[macro_export]
macro_rules! log {
    ($($args:tt)*) => {
        #[cfg(feature = "log")]
        $crate::log(&aurora_engine_types::format!("{}", format_args!($($args)*)))
    };
}

#[must_use]
pub const fn storage_byte_cost() -> u128 {
    STORAGE_PRICE_PER_BYTE
}

pub struct ECRecoverErr;

impl ECRecoverErr {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        "ERR_ECRECOVER"
    }
}

impl AsRef<[u8]> for ECRecoverErr {
    fn as_ref(&self) -> &[u8] {
        self.as_str().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurora_engine_types::types::Address;
    use aurora_engine_types::H256;

    const SIGNATURE_LENGTH: usize = 65;

    fn ecverify(hash: H256, signature: &[u8], signer: Address) -> bool {
        matches!(ecrecover(hash, signature[0..SIGNATURE_LENGTH].try_into().unwrap()), Ok(s) if s == signer)
    }

    #[test]
    fn test_ecverify() {
        let hash = H256::from_slice(
            &hex::decode("1111111111111111111111111111111111111111111111111111111111111111")
                .unwrap(),
        );
        let signature =
            &hex::decode("b9f0bb08640d3c1c00761cdd0121209268f6fd3816bc98b9e6f3cc77bf82b69812ac7a61788a0fdc0e19180f14c945a8e1088a27d92a74dce81c0981fb6447441b")
                .unwrap();
        let signer = Address::try_from_slice(
            &hex::decode("1563915e194D8CfBA1943570603F7606A3115508").unwrap(),
        )
        .unwrap();
        assert!(ecverify(hash, signature, signer));
    }
}

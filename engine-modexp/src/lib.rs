#![cfg_attr(not(feature = "std"), no_std)]
// All `as` conversions in this code base have been carefully reviewed
// and are safe.
#![allow(clippy::as_conversions)]

mod arith;
mod maybe_std;
mod mpnat;

use maybe_std::Vec;

/// Computes `(base ^ exp) % modulus`, where all values are given as big-endian
/// encoded bytes.
pub fn modexp(base: &[u8], exp: &[u8], modulus: &[u8]) -> Vec<u8> {
    let mut x = mpnat::MPNat::from_big_endian(base);
    let m = mpnat::MPNat::from_big_endian(modulus);
    if m.digits.len() == 1 && m.digits[0] == 0 {
        return Vec::new();
    }
    let result = x.modpow(exp, &m);
    result.to_big_endian()
}

#[cfg(feature = "bench")]
pub fn modexp_ibig(base: &[u8], exp: &[u8], modulus: &[u8]) -> Vec<u8> {
    use num::Zero;

    let base = ibig::UBig::from_be_bytes(base);
    let modulus = ibig::UBig::from_be_bytes(modulus);
    if modulus.is_zero() {
        return Vec::new();
    }
    let exponent = ibig::UBig::from_be_bytes(exp);
    let ring = ibig::modular::ModuloRing::new(&modulus);
    let result = ring.from(base).pow(&exponent);
    result.residue().to_be_bytes()
}

#[cfg(feature = "bench")]
pub fn modexp_num(base: &[u8], exp: &[u8], modulus: &[u8]) -> Vec<u8> {
    use num::Zero;

    let base = num::BigUint::from_bytes_be(base);
    let modulus = num::BigUint::from_bytes_be(modulus);
    if modulus.is_zero() {
        return Vec::new();
    }
    let exponent = num::BigUint::from_bytes_be(exp);
    base.modpow(&exponent, &modulus).to_bytes_be()
}

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
    _input: &[u8],
    _target_gas: Option<u64>,
    _context: &Context,
) -> Option<PrecompileResult> {
    if address == Address::from_low_u64_be(1) {
        None // TODO: implement ecrecover()
    } else if address == Address::from_low_u64_be(2) {
        None // TODO: implement sha256()
    } else if address == Address::from_low_u64_be(3) {
        None // TODO: implement ripemd160()
    } else if address == Address::from_low_u64_be(4) {
        None // TODO: implement identity()
    } else if address == Address::from_low_u64_be(5) {
        None // TODO: implement modexp()
    } else if address == Address::from_low_u64_be(6) {
        None // TODO: implement alt_bn128_add()
    } else if address == Address::from_low_u64_be(7) {
        None // TODO: implement alt_bn128_mul()
    } else if address == Address::from_low_u64_be(8) {
        None // TODO: implement alt_bn128_pair()
    } else if address == Address::from_low_u64_be(9) {
        None // TODO: implement blake2f()
    } else {
        None // not supported
    }
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
#[allow(dead_code)]
fn ecrecover(_hash: H256, _v: u8, _r: H256, _s: H256) -> Address {
    Address::zero() // TODO: implement ECRECOVER
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
#[allow(dead_code)]
fn sha256(_input: Vec<u8>) -> H256 {
    H256::zero() // TODO: implement SHA-256
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
#[allow(dead_code)]
fn ripemd160(_input: Vec<u8>) -> H160 {
    H160::zero() // TODO: implement RIPEMD-160
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
#[allow(dead_code)]
fn identity(input: Vec<u8>) -> Vec<u8> {
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
mod tests {}

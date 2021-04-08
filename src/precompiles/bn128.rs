use crate::prelude::{U256, Vec};

/// See: https://eips.ethereum.org/EIPS/eip-196
/// See: https://etherscan.io/address/0000000000000000000000000000000000000006
#[allow(dead_code)]
pub(crate) fn alt_bn128_add(_ax: U256, _ay: U256, _bx: U256, _by: U256) {
    // TODO: implement alt_bn128_add
}

/// See: https://eips.ethereum.org/EIPS/eip-196
/// See: https://etherscan.io/address/0000000000000000000000000000000000000007
#[allow(dead_code)]
pub(crate) fn alt_bn128_mul(_x: U256, _y: U256, _scalar: U256) {
    // TODO: implement alt_bn128_mul
}

/// See: https://eips.ethereum.org/EIPS/eip-197
/// See: https://etherscan.io/address/0000000000000000000000000000000000000008
#[allow(dead_code)]
pub(crate) fn alt_bn128_pair(_input: Vec<u8>) -> U256 {
    U256::zero() // TODO: implement alt_bn128_pairing
}
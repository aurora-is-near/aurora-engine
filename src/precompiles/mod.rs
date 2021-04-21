mod blake2;
mod bn128;
mod hash;
mod modexp;
mod secp256k1;
mod util;

pub(crate) use crate::precompiles::secp256k1::ecrecover;
use crate::prelude::{Address, Vec};
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
    target_gas: Option<u64>,
    _context: &Context,
) -> Option<PrecompileResult> {
    match address.to_low_u64_be() {
        1 => Some(Ok((
            ExitSucceed::Returned,
            secp256k1::ecrecover_raw(input).as_bytes().to_vec(),
            0,
        ))),
        2 => Some(Ok((
            ExitSucceed::Returned,
            hash::sha256(input).as_bytes().to_vec(),
            0,
        ))),
        3 => Some(Ok((
            ExitSucceed::Returned,
            hash::ripemd160(input).as_bytes().to_vec(),
            0,
        ))),
        4 => Some(Ok((ExitSucceed::Returned, identity(input).to_vec(), 0))),
        5 => match modexp::modexp(input, target_gas) {
            Ok(r) => Some(Ok((ExitSucceed::Returned, r, 0))),
            Err(e) => Some(Err(e)),
        },
        6 => match bn128::alt_bn128_add(input, target_gas) {
            Ok(v) => Some(Ok((ExitSucceed::Returned, v, 0))),
            Err(e) => Some(Err(e)),
        },
        7 => match bn128::alt_bn128_mul(input, target_gas) {
            Ok(v) => Some(Ok((ExitSucceed::Returned, v, 0))),
            Err(e) => Some(Err(e)),
        },
        8 => match bn128::alt_bn128_pair(input, target_gas) {
            Ok(v) => Some(Ok((ExitSucceed::Returned, v, 0))),
            Err(e) => Some(Err(e)),
        },
        9 => Some(Ok((ExitSucceed::Returned, blake2::blake2f(input), 0))),
        // Not supported.
        _ => None,
    }
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
/// See: https://etherscan.io/address/0000000000000000000000000000000000000004
fn identity(input: &[u8]) -> &[u8] {
    input
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity() {
        assert_eq!(identity(b""), b"")
    }
}

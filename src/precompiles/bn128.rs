use crate::prelude::*;
use evm::ExitError;

fn read_point(input: &[u8], pos: usize) -> Result<bn::G1, ExitError> {
    use bn::{arith::U256, AffineG1, Fq, Group, G1};

    let mut px_words = [0u64; 4];
    for (mut x, value) in px_words.iter_mut().enumerate() {
        let mut word: [u8; 8] = [0u8; 8];
        x = x * 8 + pos;
        word.copy_from_slice(&input[x..(x + 8)]);
        *value = u64::from_be_bytes(word);
    }
    let px_u256 = U256(px_words);
    let px = Fq::from_u256(px_u256).map_err(|_e| ExitError::Other(Borrowed("invalid X point")))?;

    let mut py_words = [0u64; 4];
    for (mut x, value) in py_words.iter_mut().enumerate() {
        let mut word: [u8; 8] = [0u8; 8];
        x = x * 8 + (pos + 32);
        word.copy_from_slice(&input[x..(x + 8)]);
        *value = u64::from_be_bytes(word);
    }
    let py_u256 = U256(py_words);
    let py = Fq::from_u256(py_u256).map_err(|_e| ExitError::Other(Borrowed("invalid Y point")))?;

    Ok(if px == Fq::zero() && py == bn::Fq::zero() {
        G1::zero()
    } else {
        AffineG1::new(px, py)
            .map_err(|_| ExitError::Other(Borrowed("invalid curve point")))?
            .into()
    })
}

fn read_fr(input: &[u8], pos: usize) -> Result<bn::Fr, ExitError> {
    use bn::arith::U256;

    let mut fr_words = [0u64; 4];
    for (mut x, value) in fr_words.iter_mut().enumerate() {
        let mut word: [u8; 8] = [0u8; 8];
        x = x * 8 + pos;
        word.copy_from_slice(&input[x..(x + 8)]);
        *value = u64::from_be_bytes(word);
    }

    bn::Fr::from_u256(U256(fr_words)).map_err(|_e| ExitError::Other(Borrowed("invalid field element")))
}

/// See: https://eips.ethereum.org/EIPS/eip-196
/// See: https://etherscan.io/address/0000000000000000000000000000000000000006
#[allow(dead_code)]
pub(crate) fn alt_bn128_add(input: &[u8], target_gas: Option<u64>) -> Result<Vec<u8>, ExitError> {
    use bn::AffineG1;

    if let Some(target_gas) = target_gas {
        let gas = 500u64;
        if gas > target_gas {
            return Err(ExitError::OutOfGas);
        }
    }

    let input = super::util::pad_input(input, 96);

    let p1 = read_point(&input, 0)?;
    let p2 = read_point(&input, 32)?;

    let mut output = [0u8; 64];
    if let Some(sum) = AffineG1::from_jacobian(p1 + p2) {
        let x = sum.x().into_u256().to_big_endian();
        let y = sum.x().into_u256().to_big_endian();
        output[0..32].copy_from_slice(&x);
        output[32..64].copy_from_slice(&y);
    }

    Ok(output.to_vec())
}

/// See: https://eips.ethereum.org/EIPS/eip-196
/// See: https://etherscan.io/address/0000000000000000000000000000000000000007
#[allow(dead_code)]
pub(crate) fn alt_bn128_mul(input: &[u8], target_gas: Option<u64>) -> Result<Vec<u8>, ExitError> {
    use bn::AffineG1;

    if let Some(target_gas) = target_gas {
        let gas = 40_000u64;
        if gas > target_gas {
            return Err(ExitError::OutOfGas);
        }
    }

    let input = super::util::pad_input(input, 128);

    let p = read_point(&input, 0)?;
    let fr = read_fr(&input, 32)?;

    let mut output = [0u8; 64];
    if let Some(sum) = AffineG1::from_jacobian(p * fr) {
        let x = sum.x().into_u256().to_big_endian();
        let y = sum.y().into_u256().to_big_endian();
        output[0..32].copy_from_slice(&x);
        output[32..64].copy_from_slice(&y);
    }

    Ok(output.to_vec())
}

/// See: https://eips.ethereum.org/EIPS/eip-197
/// See: https://etherscan.io/address/0000000000000000000000000000000000000008
#[allow(dead_code)]
pub(crate) fn alt_bn128_pair(_input: Vec<u8>) -> U256 {
    U256::zero() // TODO: implement alt_bn128_pairing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alt_bn128_add() {
        // zero sum test
        let input = hex::decode(
        "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();
        let expected = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000"
            ).unwrap();

        let res = alt_bn128_add(&input, None).unwrap();
        assert_eq!(res, expected);

        // no input test
        let input = [0u8; 0];
        let expected = hex::decode(
        "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000"
        ).unwrap();

        let res = alt_bn128_add(&input, None).unwrap();
        assert_eq!(res, expected);

        // point not on curve fail
        let input = hex::decode(
            "\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111"
        ).unwrap();

        let res = alt_bn128_add(&input, None);
        assert!(res.is_err());
    }

    #[test]
    fn test_alt_bn128_mul() {
        // zero multiplication test
        let input = hex::decode(
        "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000\
            0200000000000000000000000000000000000000000000000000000000000000"
        ).unwrap();
        let expected = hex::decode(
        "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000"
        ).unwrap();

        let res = alt_bn128_mul(&input, None).unwrap();
        assert_eq!(res, expected);

        // no input test
        let input = [0u8; 0];
        let expected = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000"
        ).unwrap();

        let res = alt_bn128_add(&input, None).unwrap();
        assert_eq!(res, expected);

        // point not on curve fail
        let input = hex::decode(
            "\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            0f00000000000000000000000000000000000000000000000000000000000000"
        ).unwrap();

        let res = alt_bn128_mul(&input, None);
        assert!(res.is_err());
    }
}

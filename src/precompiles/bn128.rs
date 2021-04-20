use crate::prelude::*;
use evm::ExitError;

fn read_point(input: &[u8], pos: usize) -> Result<bn::G1, &'static str> {
    use bn::{arith::U256, AffineG1, Fq, Group, G1};
    let mut px_words = [0u64; 4];
    for (mut x, value) in px_words.iter_mut().enumerate() {
        let mut word: [u8; 8] = [0u8; 8];
        x = x * 8 + (64 * pos);
        word.copy_from_slice(&input[x..(x + 8)]);
        *value = u64::from_be_bytes(word);
    }
    let px_u256 = U256(px_words);
    let px = Fq::from_u256(px_u256).map_err(|_e| "invalid X point")?;

    let mut py_words = [0u64; 4];
    for (mut x, value) in py_words.iter_mut().enumerate() {
        let mut word: [u8; 8] = [0u8; 8];
        x = x * 8 + (64 * pos) + 32;
        word.copy_from_slice(&input[x..(x + 8)]);
        *value = u64::from_be_bytes(word);
    }
    let py_u256 = U256(py_words);
    let py = Fq::from_u256(py_u256).map_err(|_e| "invalid Y point")?;

    Ok(if px == Fq::zero() && py == bn::Fq::zero() {
        G1::zero()
    } else {
        AffineG1::new(px, py)
            .map_err(|_| "invalid curve point")?
            .into()
    })
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

    // pad if necessary
    let input = if input.len() < 128 {
        let mut input = input.to_vec();
        for x in 0..(128 - input.len()) {
            input.push(0);
        }
        input
    } else {
        input.to_vec()
    };

    let p1 = read_point(&input, 0).map_err(|e| ExitError::Other(Borrowed(e)))?;
    let p2 = read_point(&input, 1).map_err(|e| ExitError::Other(Borrowed(e)))?;

    let mut output = [0u8; 64];
    if let Some(sum) = AffineG1::from_jacobian(p1 + p2) {
        let x = sum.x().into_u256().to_big_endian();
        let y = sum.x().into_u256().to_big_endian();
        output[0..32].copy_from_slice(&x);
        output[32..64].copy_from_slice(&y);
    }

    Ok(output.to_vec())
}

// /// See: https://eips.ethereum.org/EIPS/eip-196
// /// See: https://etherscan.io/address/0000000000000000000000000000000000000007
// #[allow(dead_code)]
// pub(crate) fn alt_bn128_mul(_x: U256, _y: U256, _scalar: U256) {
//     let x = U256::from_big_endian(&input[0..32]);
//     let y = U256::from_big_endian(&input[32..64]);
// }

/// See: https://eips.ethereum.org/EIPS/eip-197
/// See: https://etherscan.io/address/0000000000000000000000000000000000000008
#[allow(dead_code)]
pub(crate) fn alt_bn128_pair(_input: Vec<u8>) -> U256 {
    U256::zero() // TODO: implement alt_bn128_pairing
}

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
        let mut input = [0u8; 0];
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
}

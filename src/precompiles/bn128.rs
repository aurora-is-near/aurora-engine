use crate::prelude::*;
use evm::ExitError;

fn read_point(input: &[u8], pos: usize) -> Result<bn::G1, ExitError> {
    use bn::{AffineG1, Fq, Group, G1};

    let px = Fq::from_slice(&input[pos..(pos + 32)])
        .map_err(|_e| ExitError::Other(Borrowed("invalid `x` point")))?;
    let py = Fq::from_slice(&input[(pos + 32)..(pos + 64)])
        .map_err(|_e| ExitError::Other(Borrowed("invalid `y` point")))?;

    Ok(if px == Fq::zero() && py == bn::Fq::zero() {
        G1::zero()
    } else {
        AffineG1::new(px, py)
            .map_err(|_| ExitError::Other(Borrowed("invalid curve point")))?
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
    let fr = bn::Fr::from_slice(&input[32..64])
        .map_err(|_e| ExitError::Other(Borrowed("invalid field element")))?;

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
pub(crate) fn alt_bn128_pair(input: &[u8], target_gas: Option<u64>) -> Result<Vec<u8>, ExitError> {
    use bn::{arith::U256, AffineG1, AffineG2, Fq, Fq2, Group, Gt, G1, G2};

    if let Some(target_gas) = target_gas {
        let gas = input.len() as u64 / 192u64;
        if gas > target_gas {
            return Err(ExitError::OutOfGas);
        }
    }

    if input.len() % 192 != 0 {
        return Err(ExitError::Other(Borrowed(
            "input length invalid, must be multiple of 192",
        )));
    }

    let ret = if input.is_empty() {
        U256::one()
    } else {
        let elements = input.len() / 192;
        let mut vals = Vec::with_capacity(elements);

        for idx in 0..elements {
            let ax = Fq::from_slice(&input[(idx * 192)..(idx * 192 + 32)])
                .map_err(|_e| ExitError::Other(Borrowed("invalid `a` argument, `x` coordinate")))?;
            let ay = Fq::from_slice(&input[(idx * 192 + 32)..(idx * 192 + 64)])
                .map_err(|_e| ExitError::Other(Borrowed("invalid `a` argument, `y` coordinate")))?;
            let bay = Fq::from_slice(&input[(idx * 192 + 64)..(idx * 192 + 96)])
                .map_err(|_e| ExitError::Other(Borrowed("invalid `a` argument, `x` coordinate")))?;
            let bax = Fq::from_slice(&input[(idx * 192 + 96)..(idx * 192 + 128)])
                .map_err(|_e| ExitError::Other(Borrowed("invalid `a` argument, `x` coordinate")))?;
            let bby = Fq::from_slice(&input[(idx * 192 + 128)..(idx * 192 + 160)])
                .map_err(|_e| ExitError::Other(Borrowed("invalid `a` argument, `x` coordinate")))?;
            let bbx = Fq::from_slice(&input[(idx * 192 + 160)..(idx * 192 + 192)])
                .map_err(|_e| ExitError::Other(Borrowed("invalid `a` argument, `x` coordinate")))?;

            let a = {
                if ax.is_zero() && ay.is_zero() {
                    G1::zero()
                } else {
                    G1::from(AffineG1::new(ax, ay).map_err(|_e| {
                        ExitError::Other(Borrowed("invalid `a` argument, not on curve"))
                    })?)
                }
            };
            let b = {
                let ba = Fq2::new(bax, bay);
                let bb = Fq2::new(bbx, bby);

                if ba.is_zero() && bb.is_zero() {
                    G2::zero()
                } else {
                    G2::from(AffineG2::new(ba, bb).map_err(|_e| {
                        ExitError::Other(Borrowed("invalid `b` argument, not on curve"))
                    })?)
                }
            };
            vals.push((a, b))
        }

        let mul = vals
            .into_iter()
            .fold(Gt::one(), |s, (a, b)| s * bn::pairing(a, b));

        if mul == Gt::one() {
            U256::one()
        } else {
            U256::zero()
        }
    };

    Ok(ret.to_big_endian().to_vec())
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
            0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();

        let res = alt_bn128_add(&input, None).unwrap();
        assert_eq!(res, expected);

        // no input test
        let input = [0u8; 0];
        let expected = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();

        let res = alt_bn128_add(&input, None).unwrap();
        assert_eq!(res, expected);

        // point not on curve fail
        let input = hex::decode(
            "\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111",
        )
        .unwrap();

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
            0200000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();
        let expected = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();

        let res = alt_bn128_mul(&input, None).unwrap();
        assert_eq!(res, expected);

        // no input test
        let input = [0u8; 0];
        let expected = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();

        let res = alt_bn128_add(&input, None).unwrap();
        assert_eq!(res, expected);

        // point not on curve fail
        let input = hex::decode(
            "\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            0f00000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();

        let res = alt_bn128_mul(&input, None);
        assert!(res.is_err());
    }

    #[test]
    fn test_alt_bn128_pair() {
        // no input test
        let input = [0u8; 0];
        let expected = hex::decode(
            "0000000000000000000000000000000000000000000000000000000000000001"
        ).unwrap();

        let res = alt_bn128_pair(&input, None).unwrap();
        assert_eq!(res, expected);

        // point not on curve fail
        let input = hex::decode("\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111"
        ).unwrap();

        let res = alt_bn128_pair(&input, None);
        assert!(res.is_err());

        // invalid input length
        let input = hex::decode("\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            111111111111111111111111111111\
        ").unwrap();

        let res = alt_bn128_pair(&input, None);
        assert!(res.is_err());
    }
}

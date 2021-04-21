use crate::prelude::*;
use evm::ExitError;

fn read_point(input: &[u8], pos: usize) -> Result<bn::G1, ExitError> {
    use bn::{AffineG1, Fq, Group, G1};

    let mut px_buf = [0u8; 32];
    px_buf.copy_from_slice(&input[pos..(pos + 32)]);
    let px =
        Fq::interpret(&px_buf).map_err(|_e| ExitError::Other(Borrowed("invalid `x` point")))?;

    let mut py_buf = [0u8; 32];
    py_buf.copy_from_slice(&input[(pos + 32)..(pos + 64)]);
    let py =
        Fq::interpret(&py_buf).map_err(|_e| ExitError::Other(Borrowed("invalid `y` point")))?;

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

    let input = super::util::pad_input(input, 128);

    let p1 = read_point(&input, 0)?;
    let p2 = read_point(&input, 64)?;

    let mut output = [0u8; 64];
    if let Some(sum) = AffineG1::from_jacobian(p1 + p2) {
        let x = sum.x().into_u256().to_big_endian();
        let y = sum.y().into_u256().to_big_endian();
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
    let mut fr_buf = [0u8; 32];
    fr_buf.copy_from_slice(&input[64..96]);
    let fr = bn::Fr::interpret(&fr_buf)
        .map_err(|_e| ExitError::Other(Borrowed("invalid field element")))?;

    let mut output = [0u8; 64];
    if let Some(mul) = AffineG1::from_jacobian(p * fr) {
        let x = mul.x().into_u256().to_big_endian();
        let y = mul.y().into_u256().to_big_endian();
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
            let mut buf = [0u8; 32];

            buf.copy_from_slice(&input[(idx * 192)..(idx * 192 + 32)]);
            let ax = Fq::interpret(&buf)
                .map_err(|_e| ExitError::Other(Borrowed("invalid `a` argument, `x` coordinate")))?;
            buf.copy_from_slice(&input[(idx * 192 + 32)..(idx * 192 + 64)]);
            let ay = Fq::interpret(&buf)
                .map_err(|_e| ExitError::Other(Borrowed("invalid `a` argument, `y` coordinate")))?;
            buf.copy_from_slice(&input[(idx * 192 + 64)..(idx * 192 + 96)]);
            let bay = Fq::interpret(&buf)
                .map_err(|_e| ExitError::Other(Borrowed("invalid `a` argument, `x` coordinate")))?;
            buf.copy_from_slice(&input[(idx * 192 + 96)..(idx * 192 + 128)]);
            let bax = Fq::interpret(&buf)
                .map_err(|_e| ExitError::Other(Borrowed("invalid `a` argument, `x` coordinate")))?;
            buf.copy_from_slice(&input[(idx * 192 + 128)..(idx * 192 + 160)]);
            let bby = Fq::interpret(&buf)
                .map_err(|_e| ExitError::Other(Borrowed("invalid `a` argument, `x` coordinate")))?;
            buf.copy_from_slice(&input[(idx * 192 + 160)..(idx * 192 + 192)]);
            let bbx = Fq::interpret(&buf)
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
        let input = hex::decode(
            "\
             18b18acfb4c2c30276db5411368e7185b311dd124691610c5d3b74034e093dc9\
             063c909c4720840cb5134cb9f59fa749755796819658d32efc0d288198f37266\
             07c2b7f58a84bd6145f00c9c2bc0bb1a187f20ff2c92963a88019e7c6a014eed\
             06614e20c147e940f2d70da3f74c9a17df361706a4485c742bd6788478fa17d7",
        )
        .unwrap();
        let expected = hex::decode(
            "\
            2243525c5efd4b9c3d3c45ac0ca3fe4dd85e830a4ce6b65fa1eeaee202839703\
            301d1d33be6da8e509df21cc35964723180eed7532537db9ae5e7d48f195c915",
        )
        .unwrap();

        let res = alt_bn128_add(&input, None).unwrap();
        assert_eq!(res, expected);

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
        let input = hex::decode(
            "\
            2bd3e6d0f3b142924f5ca7b49ce5b9d54c4703d7ae5648e61d02268b1a0a9fb7\
            21611ce0a6af85915e2f1d70300909ce2e49dfad4a4619c8390cae66cefdb204\
            00000000000000000000000000000000000000000000000011138ce750fa15c2",
        )
        .unwrap();
        let expected = hex::decode(
            "\
            070a8d6a982153cae4be29d434e8faef8a47b274a053f5a4ee2a6c9c13c31e5c\
            031b8ce914eba3a9ffb989f9cdd5b0f01943074bf4f0f315690ec3cec6981afc",
        )
        .unwrap();

        let res = alt_bn128_mul(&input, None).unwrap();
        assert_eq!(res, expected);

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
        let expected =
            hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();

        let res = alt_bn128_pair(&input, None).unwrap();
        assert_eq!(res, expected);

        // point not on curve fail
        let input = hex::decode(
            "\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111",
        )
        .unwrap();

        let res = alt_bn128_pair(&input, None);
        assert!(res.is_err());

        // invalid input length
        let input = hex::decode(
            "\
            1111111111111111111111111111111111111111111111111111111111111111\
            1111111111111111111111111111111111111111111111111111111111111111\
            111111111111111111111111111111\
        ",
        )
        .unwrap();

        let res = alt_bn128_pair(&input, None);
        assert!(res.is_err());
    }
}

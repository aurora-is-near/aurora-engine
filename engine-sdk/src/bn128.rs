use aurora_engine_types::Vec;

#[cfg(feature = "contract")]
use super::exports;

#[derive(Debug)]
pub enum BnError {
    Field(bn::FieldError),
    Scalar(bn::FieldError),
    G1(bn::GroupError),
    G2(bn::GroupError),
}

impl From<bn::FieldError> for BnError {
    fn from(err: bn::FieldError) -> Self {
        Self::Field(err)
    }
}

#[cfg(feature = "contract")]
pub fn alt_bn128_g1_sum(left: [u8; 64], right: [u8; 64]) -> Result<[u8; 64], BnError> {
    use aurora_engine_types::U256;
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
        Ok(output)
    }
}

#[cfg(not(feature = "contract"))]
pub fn alt_bn128_g1_sum(left: [u8; 64], right: [u8; 64]) -> Result<[u8; 64], BnError> {
    let p1 = read_bn_g1(left)?;
    let p2 = read_bn_g1(right)?;

    let mut output = [0u8; 0x40];
    if let Some(sum) = bn::AffineG1::from_jacobian(p1 + p2) {
        sum.x().to_big_endian(&mut output[0x00..0x20])?;
        sum.y().to_big_endian(&mut output[0x20..0x40])?;
    }

    Ok(output)
}

#[cfg(feature = "contract")]
pub fn alt_bn128_g1_scalar_multiple(g1: [u8; 64], fr: [u8; 32]) -> Result<[u8; 64], BnError> {
    use aurora_engine_types::U256;
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
        Ok(output)
    }
}

#[cfg(not(feature = "contract"))]
pub fn alt_bn128_g1_scalar_multiple(
    point: [u8; 64],
    mut scalar: [u8; 32],
) -> Result<[u8; 64], BnError> {
    let p = read_bn_g1(point)?;
    scalar.reverse(); // To little-endian
    let scalar = bn::Fr::from_slice(&scalar).map_err(BnError::Scalar)?;

    let mut output = [0u8; 0x40];
    if let Some(result) = bn::AffineG1::from_jacobian(p * scalar) {
        result.x().to_big_endian(&mut output[0x00..0x20])?;
        result.y().to_big_endian(&mut output[0x20..0x40])?;
    }

    Ok(output)
}

#[cfg(feature = "contract")]
pub fn alt_bn128_pairing<I>(pairs: I) -> Result<bool, BnError>
where
    I: ExactSizeIterator<Item = ([u8; 64], [u8; 128])>,
{
    let n = pairs.len();
    let mut bytes = Vec::with_capacity(n * 6 * 32);
    for (g1, g2) in pairs {
        bytes.extend_from_slice(&g1);
        bytes.extend_from_slice(&g2);
    }

    let value_ptr = bytes.as_ptr() as u64;
    let value_len = bytes.len() as u64;

    let result = unsafe { exports::alt_bn128_pairing_check(value_len, value_ptr) };

    Ok(result == 1)
}

#[cfg(not(feature = "contract"))]
pub fn alt_bn128_pairing<I>(pairs: I) -> Result<bool, BnError>
where
    I: ExactSizeIterator<Item = ([u8; 64], [u8; 128])>,
{
    let mut vals = Vec::with_capacity(pairs.len());
    for (g1, g2) in pairs {
        let g1 = read_bn_g1(g1)?;
        let g2 = read_bn_g2(g2)?;
        vals.push((g1, g2));
    }
    let gt = bn::pairing_batch(&vals);
    Ok(gt == bn::Gt::one())
}

#[cfg(not(feature = "contract"))]
fn read_bn_g1(mut x: [u8; 64]) -> Result<bn::G1, BnError> {
    // To little-endian
    x.chunks_mut(0x20).for_each(<[u8]>::reverse);

    let px = bn::Fq::from_slice(&x[0x00..0x20])?;
    let py = bn::Fq::from_slice(&x[0x20..0x40])?;

    Ok(if px.is_zero() && py.is_zero() {
        <bn::G1 as bn::Group>::zero()
    } else {
        bn::AffineG1::new(px, py).map_err(BnError::G1)?.into()
    })
}

#[cfg(not(feature = "contract"))]
fn read_bn_g2(mut x: [u8; 0x80]) -> Result<bn::G2, BnError> {
    // To little-endian
    x.chunks_mut(0x20).for_each(<[u8]>::reverse);

    let mut v = [bn::Fq::zero(); 4];
    for (x, p) in x.chunks(0x20).zip(v.iter_mut()) {
        *p = bn::Fq::from_slice(x)?;
    }

    let ba = bn::Fq2::new(v[0], v[1]);
    let bb = bn::Fq2::new(v[2], v[3]);

    Ok(if ba.is_zero() && bb.is_zero() {
        <bn::G2 as bn::Group>::zero()
    } else {
        bn::AffineG2::new(ba, bb).map_err(BnError::G2)?.into()
    })
}

use super::BnError;

impl From<bn::FieldError> for BnError {
    fn from(err: bn::FieldError) -> Self {
        Self::Field(err)
    }
}

impl From<bn::GroupError> for BnError {
    fn from(err: bn::GroupError) -> Self {
        Self::Group(err)
    }
}

fn read_bn_g1(mut x: [u8; 64]) -> Result<bn::G1, BnError> {
    // To little-endian
    x.chunks_mut(0x20).for_each(<[u8]>::reverse);

    let px = bn::Fq::from_slice(&x[0x00..0x20])?;
    let py = bn::Fq::from_slice(&x[0x20..0x40])?;

    Ok(if px.is_zero() && py.is_zero() {
        <bn::G1 as bn::Group>::zero()
    } else {
        bn::AffineG1::new(px, py)?.into()
    })
}

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
        bn::AffineG2::new(ba, bb)?.into()
    })
}

/// Big-endian inputs and outputs
pub fn g1_sum(left: [u8; 64], right: [u8; 64]) -> Result<[u8; 64], BnError> {
    let p1 = read_bn_g1(left)?;
    let p2 = read_bn_g1(right)?;

    let mut output = [0u8; 0x40];
    if let Some(sum) = bn::AffineG1::from_jacobian(p1 + p2) {
        sum.x().to_big_endian(&mut output[0x00..0x20])?;
        sum.y().to_big_endian(&mut output[0x20..0x40])?;
    }

    Ok(output)
}

/// Big-endian inputs and outputs
pub fn g1_scalar_multiple(point: [u8; 64], mut scalar: [u8; 32]) -> Result<[u8; 64], BnError> {
    let p = read_bn_g1(point)?;
    scalar.reverse(); // To little-endian
    let scalar = bn::Fr::from_slice(&scalar)?;

    let mut output = [0u8; 0x40];
    if let Some(result) = bn::AffineG1::from_jacobian(p * scalar) {
        result.x().to_big_endian(&mut output[0x00..0x20])?;
        result.y().to_big_endian(&mut output[0x20..0x40])?;
    }
    Ok(output)
}

/// Big-endian inputs
pub fn pairing<I>(pairs: I) -> Result<bool, BnError>
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

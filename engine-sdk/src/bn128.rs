use aurora_engine_types::{Cow, Vec};

#[cfg(feature = "contract")]
use super::exports;

/// FQ_LEN specifies the number of bytes needed to represent a  Fq element.
/// This is an element in the base field of `bn254`.
///
/// Note: The base field is used to define G1 and G2 elements.
const FQ_LEN: usize = 32;

/// SCALAR_LEN specifies the number of bytes needed to represent a Fr element.
/// This is an element in the scalar field of BN254.
const SCALAR_LEN: usize = 32;

/// FQ2_LEN specifies the number of bytes needed to represent a  Fq^2 element.
///
/// Note: This is the quadratic extension of Fq, and by definition
/// means we need 2 Fq elements.
const FQ2_LEN: usize = 2 * FQ_LEN;

/// G1_LEN specifies the number of bytes needed to represent a G1 element.
///
/// Note: A G1 element contains 2 Fq elements.
pub const G1_LEN: usize = 2 * FQ_LEN;
/// G2_LEN specifies the number of bytes needed to represent a G2 element.
///
/// Note: A G2 element contains 2 Fq^2 elements.
pub const G2_LEN: usize = 2 * FQ2_LEN;

/// Input length for the add operation.
/// `ADD` takes two uncompressed G1 points (64 bytes each).
pub const ADD_INPUT_LEN: usize = 2 * G1_LEN;

/// Input length for the multiplication operation.
/// `MUL` takes an uncompressed G1 point (64 bytes) and scalar (32 bytes).
pub const MUL_INPUT_LEN: usize = G1_LEN + SCALAR_LEN;

/// Pair element length.
/// `PAIR` elements are composed of an uncompressed G1 point (64 bytes) and an uncompressed G2 point
/// (128 bytes).
pub const PAIR_ELEMENT_LEN: usize = G1_LEN + G2_LEN;

/// Right-pads the given slice with zeroes until `LEN`.
/// Returns the first `LEN` bytes if it does not need padding.
#[inline]
pub fn right_pad<const LEN: usize>(data: &[u8]) -> Cow<'_, [u8; LEN]> {
    if let Some((head, _)) = data.split_first_chunk::<LEN>() {
        Cow::Borrowed(head)
    } else {
        let mut padded = [0; LEN];
        padded[..data.len()].copy_from_slice(data);
        Cow::Owned(padded)
    }
}

#[cfg(not(feature = "contract"))]
mod utils {
    use super::{Bn254Error, FQ2_LEN, FQ_LEN, G1_LEN};

    use ark_bn254::{Fq, Fq2, G1Affine, G1Projective, G2Affine};
    use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup};
    use ark_ff::Zero;
    use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};

    /// Reads a single `Fq` field element from the input slice.
    ///
    /// Takes a byte slice and attempts to interpret the first 32 bytes as an
    /// elliptic curve field element. Returns an error if the bytes do not form
    /// a valid field element.
    #[inline]
    fn read_fq(input_be: &[u8]) -> Result<Fq, Bn254Error> {
        if input_be.len() != FQ_LEN {
            return Err(Bn254Error::InvalidFqLength);
        }

        let mut input_le = [0u8; FQ_LEN];
        input_le.copy_from_slice(input_be);

        // Reverse in-place to convert from big-endian to little-endian.
        input_le.reverse();

        Fq::deserialize_uncompressed(&input_le[..]).map_err(|_| Bn254Error::FieldPointNotAMember)
    }

    /// Reads a Fq2 (quadratic extension field element) from the input slice.
    ///
    /// Parses two consecutive Fq field elements as the real and imaginary parts
    /// of a Fq2 element.
    /// The second component is parsed before the first, ie if it represents an
    /// element in Fq2 as (x,y) -- `y` is parsed before `x`
    #[inline]
    fn read_fq2(input: &[u8]) -> Result<Fq2, Bn254Error> {
        let y = read_fq(&input[..FQ_LEN])?;
        let x = read_fq(&input[FQ_LEN..FQ2_LEN])?;

        Ok(Fq2::new(x, y))
    }

    /// Creates a new `G1` point from the given `x` and `y` coordinates.
    ///
    /// Constructs a point on the G1 curve from its affine coordinates.
    ///
    /// Note: The point at infinity which is represented as (0,0) is
    /// handled specifically because `AffineG1` is not capable of
    /// representing such a point.
    /// In particular, when we convert from `AffineG1` to `G1`, the point
    /// will be (0,0,1) instead of (0,1,0)
    #[inline]
    fn new_g1_point(px: Fq, py: Fq) -> Result<G1Affine, Bn254Error> {
        if px.is_zero() && py.is_zero() {
            Ok(G1Affine::zero())
        } else {
            // We cannot use `G1Affine::new` because that triggers an assert if the point is not on the curve.
            let point = G1Affine::new_unchecked(px, py);
            if !point.is_on_curve() || !point.is_in_correct_subgroup_assuming_on_curve() {
                return Err(Bn254Error::AffineGFailedToCreate);
            }
            Ok(point)
        }
    }

    /// Creates a new `G2` point from the given Fq2 coordinates.
    ///
    /// G2 points in BN254 are defined over a quadratic extension field Fq2.
    /// This function takes two Fq2 elements representing the x and y coordinates
    /// and creates a G2 point.
    ///
    /// Note: The point at infinity which is represented as (0,0) is
    /// handled specifically because `AffineG2` is not capable of
    /// representing such a point.
    /// In particular, when we convert from `AffineG2` to `G2`, the point
    /// will be (0,0,1) instead of (0,1,0)
    #[inline]
    fn new_g2_point(x: Fq2, y: Fq2) -> Result<G2Affine, Bn254Error> {
        let point = if x.is_zero() && y.is_zero() {
            G2Affine::zero()
        } else {
            // We cannot use `G1Affine::new` because that triggers an assert if the point is not on the curve.
            let point = G2Affine::new_unchecked(x, y);
            if !point.is_on_curve() || !point.is_in_correct_subgroup_assuming_on_curve() {
                return Err(Bn254Error::AffineGFailedToCreate);
            }
            point
        };

        Ok(point)
    }

    /// Reads a G1 point from the input slice.
    ///
    /// Parses a G1 point from a byte slice by reading two consecutive field elements
    /// representing the x and y coordinates.
    #[inline]
    fn read_g1_point(input: &[u8]) -> Result<G1Affine, Bn254Error> {
        let px = read_fq(&input[0..FQ_LEN])?;
        let py = read_fq(&input[FQ_LEN..G1_LEN])?;
        new_g1_point(px, py)
    }

    /// Encodes a G1 point into a byte array.
    ///
    /// Converts a G1 point in Jacobian coordinates to affine coordinates and
    /// serializes the x and y coordinates as big-endian byte arrays.
    ///
    /// Note: If the point is the point at infinity, this function returns all zeroes.
    #[inline]
    fn encode_g1_point(point: G1Affine) -> Result<[u8; G1_LEN], Bn254Error> {
        let mut output = [0u8; G1_LEN];
        let Some((x, y)) = point.xy() else {
            return Ok(output);
        };

        let mut x_bytes = [0u8; FQ_LEN];
        x.serialize_uncompressed(&mut x_bytes[..])
            .map_err(|_| Bn254Error::FailedToSerializeX)?;

        let mut y_bytes = [0u8; FQ_LEN];
        y.serialize_uncompressed(&mut y_bytes[..])
            .map_err(|_| Bn254Error::FailedToSerializeY)?;

        // Convert to big endian by reversing the bytes.
        x_bytes.reverse();
        y_bytes.reverse();

        // Place x in the first half, y in the second half.
        output[0..FQ_LEN].copy_from_slice(&x_bytes);
        output[FQ_LEN..G1_LEN].copy_from_slice(&y_bytes);

        Ok(output)
    }

    /// Performs point addition on two G1 points.
    #[inline]
    pub fn g1_point_add(p1_bytes: &[u8], p2_bytes: &[u8]) -> Result<[u8; 64], Bn254Error> {
        let p1 = read_g1_point(p1_bytes)?;
        let p2 = read_g1_point(p2_bytes)?;

        let p1_jacobian: G1Projective = p1.into();

        let p3 = p1_jacobian + p2;
        let output = encode_g1_point(p3.into_affine())?;

        Ok(output)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Bn254Error {
    InvalidFqLength,
    FieldPointNotAMember,
    AffineGFailedToCreate,
    FailedToSerializeX,
    FailedToSerializeY,
}

impl From<Bn254Error> for Cow<'static, str> {
    fn from(err: Bn254Error) -> Self {
        match err {
            Bn254Error::InvalidFqLength => Cow::Borrowed("ERR_BN_INVALID_FQ_LEN"),
            Bn254Error::FieldPointNotAMember => Cow::Borrowed("ERR_BN_FIELD_POINT_NOT_A_MEMBER"),
            Bn254Error::AffineGFailedToCreate => Cow::Borrowed("ERR_BN_AFFINE_G_FAILED_TO_CREATE"),
            Bn254Error::FailedToSerializeX => Cow::Borrowed("ERR_BN_FAILED_SERIALIZE_X"),
            Bn254Error::FailedToSerializeY => Cow::Borrowed("ERR_BN_FAILED_SERIALIZE_Y"),
        }
    }
}

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
pub fn alt_bn128_g1_sum(left: &[u8], right: &[u8]) -> Result<[u8; 64], Bn254Error> {
    use aurora_engine_types::U256;

    let mut p1_bytes: [u8; 64] = left.try_into().map_err(|_| Bn254Error::InvalidFqLength)?;
    p1_bytes.chunks_mut(SCALAR_LEN).for_each(<[u8]>::reverse);

    let mut p2_bytes: [u8; 64] = right.try_into().map_err(|_| Bn254Error::InvalidFqLength)?;
    p2_bytes.chunks_mut(SCALAR_LEN).for_each(<[u8]>::reverse);

    // 64 bytes per G1 + 2 positive integer bytes.
    let mut bytes = [0u8; 2 + ADD_INPUT_LEN];
    bytes[1..1 + G1_LEN].copy_from_slice(&p1_bytes);
    bytes[2 + G1_LEN..].copy_from_slice(&p2_bytes);

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
pub fn alt_bn128_g1_sum(p1: &[u8], p2: &[u8]) -> Result<[u8; 64], Bn254Error> {
    utils::g1_point_add(p1, p2)
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

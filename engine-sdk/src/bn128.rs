use aurora_engine_types::Cow;

#[cfg(feature = "contract")]
use super::exports;

/// Specifies the number of bytes needed to represent a  Fq element.
/// This is an element in the base field of `bn254`.
///
/// Note: The base field is used to define G1 and G2 elements.
const FQ_LEN: usize = 32;

/// Specifies the number of bytes needed to represent a Fr element.
/// This is an element in the scalar field of BN254.
pub const SCALAR_LEN: usize = 32;

/// Specifies the number of bytes needed to represent a  Fq^2 element.
///
/// Note: This is the quadratic extension of Fq, and by definition
/// means we need 2 Fq elements.
const FQ2_LEN: usize = 2 * FQ_LEN;

/// Specifies the number of bytes needed to represent a G1 element.
///
/// Note: A G1 element contains 2 Fq elements.
pub const G1_LEN: usize = 2 * FQ_LEN;
/// Specifies the number of bytes needed to represent a G2 element.
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

#[cfg(not(feature = "contract"))]
mod utils {
    use super::{Bn254Error, Cow, FQ2_LEN, FQ_LEN, G1_LEN, SCALAR_LEN};

    use ark_bn254::{Bn254, Fq, Fq2, Fr, G1Affine, G1Projective, G2Affine};
    use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup};
    use ark_ff::{One, PrimeField, Zero};
    use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};

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
            // We cannot use `G2Affine::new` because that triggers an assert if the point is not on the curve.
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

    /// Reads a G2 point from the input slice.
    ///
    /// Parses a G2 point from a byte slice by reading four consecutive Fq field elements
    /// representing the two Fq2 coordinates (x and y) of the G2 point.
    #[inline]
    fn read_g2_point(input: &[u8]) -> Result<G2Affine, Bn254Error> {
        let ba = read_fq2(&input[0..FQ2_LEN])?;
        let bb = read_fq2(&input[FQ2_LEN..2 * FQ2_LEN])?;
        new_g2_point(ba, bb)
    }

    /// Reads a scalar from the input slice
    ///
    /// Note: The scalar does not need to be canonical.
    #[inline]
    fn read_scalar(input: &[u8]) -> Result<Fr, Bn254Error> {
        if input.len() != SCALAR_LEN {
            return Err(Bn254Error::InvalidScalarLength);
        }

        Ok(Fr::from_be_bytes_mod_order(input))
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

    /// Performs a G1 scalar multiplication.
    #[inline]
    pub fn g1_point_mul(point_bytes: &[u8], fr_bytes: &[u8]) -> Result<[u8; 64], Bn254Error> {
        let p = read_g1_point(point_bytes)?;
        let fr = read_scalar(fr_bytes)?;

        let big_int = fr.into_bigint();
        let result = p.mul_bigint(big_int);

        let output = encode_g1_point(result.into_affine())?;

        Ok(output)
    }

    /// Performs a pairing check on a list of G1 and G2 point pairs and
    /// returns true if the result is equal to the identity element.
    ///
    /// Note: If the input is empty, this function returns true.
    /// This is different to EIP2537 which disallows the empty input.
    #[inline]
    pub fn pairing_check(pairs: &[(&[u8], &[u8])]) -> Result<bool, Bn254Error> {
        let mut g1_points = Vec::with_capacity(pairs.len());
        let mut g2_points = Vec::with_capacity(pairs.len());

        for (g1_bytes, g2_bytes) in pairs {
            let g1 = read_g1_point(g1_bytes)?;
            let g2 = read_g2_point(g2_bytes)?;

            // Skip pairs where either point is at infinity
            if !g1.is_zero() && !g2.is_zero() {
                g1_points.push(g1);
                g2_points.push(g2);
            }
        }

        if g1_points.is_empty() {
            return Ok(true);
        }

        let pairing_result = Bn254::multi_pairing(&g1_points, &g2_points);
        Ok(pairing_result.0.is_one())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Bn254Error {
    InvalidFqLength,
    FieldPointNotAMember,
    AffineGFailedToCreate,
    FailedToSerializeX,
    FailedToSerializeY,
    InvalidScalarLength,
    InvalidPairLength,
}

impl From<Bn254Error> for Cow<'static, str> {
    fn from(err: Bn254Error) -> Self {
        match err {
            Bn254Error::InvalidFqLength => Cow::Borrowed("ERR_BN_INVALID_FQ_LEN"),
            Bn254Error::FieldPointNotAMember => Cow::Borrowed("ERR_BN_FIELD_POINT_NOT_A_MEMBER"),
            Bn254Error::AffineGFailedToCreate => Cow::Borrowed("ERR_BN_AFFINE_G_FAILED_TO_CREATE"),
            Bn254Error::FailedToSerializeX => Cow::Borrowed("ERR_BN_FAILED_SERIALIZE_X"),
            Bn254Error::FailedToSerializeY => Cow::Borrowed("ERR_BN_FAILED_SERIALIZE_Y"),
            Bn254Error::InvalidScalarLength => Cow::Borrowed("ERR_BN_INVALID_SCALAR_LEN"),
            Bn254Error::InvalidPairLength => Cow::Borrowed("ERR_BN_INVALID_PAIR_LEN"),
        }
    }
}

/// Adds two G1 points on the bn128 curve via NEAR host function.
#[cfg(feature = "contract")]
pub fn alt_bn128_g1_sum(input_bytes: &[u8]) -> Result<[u8; 64], Bn254Error> {
    // Buffer is: [0, P1(G1_LEN), 0, P2(G1_LEN)]
    const BUFFER_LEN: usize = 2 + G1_LEN * 2;
    // Register ID to store the result
    const REGISTER_ID: u64 = 1;

    let mut bytes = [0u8; BUFFER_LEN];

    // --- Process P1 (First 64 bytes of input) ---
    // P1.X: 1..1 + FQ_LEN
    write_reversed_chunk(&mut bytes[1..=FQ_LEN], input_bytes, 0);
    // P1.Y: 1 + FQ_LEN..1 + G1_LEN
    write_reversed_chunk(&mut bytes[1 + FQ_LEN..=G1_LEN], input_bytes, FQ_LEN);

    // --- Process P2 (Next 64 bytes of input) ---
    // P2.X
    write_reversed_chunk(
        &mut bytes[2 + G1_LEN..2 + G1_LEN + FQ_LEN],
        input_bytes,
        G1_LEN,
    );
    // P2.Y
    write_reversed_chunk(
        &mut bytes[2 + G1_LEN + FQ_LEN..],
        input_bytes,
        G1_LEN + FQ_LEN,
    );

    let value_ptr = bytes.as_ptr() as u64;
    let value_len = bytes.len() as u64;
    // Prepare output buffer
    let mut output = [0u8; G1_LEN];
    // Call the NEAR host function
    unsafe {
        exports::alt_bn128_g1_sum(value_len, value_ptr, REGISTER_ID);
        exports::read_register(REGISTER_ID, output.as_ptr() as u64);
    }

    // X = LE -> BE
    output[0..FQ_LEN].reverse();
    // Y = LE -> BE
    output[FQ_LEN..G1_LEN].reverse();
    Ok(output)
}

#[cfg(not(feature = "contract"))]
pub fn alt_bn128_g1_sum(input_bytes: &[u8]) -> Result<[u8; 64], Bn254Error> {
    let input = utils::right_pad::<ADD_INPUT_LEN>(input_bytes);
    let p1_bytes = &input[..G1_LEN];
    let p2_bytes = &input[G1_LEN..];
    utils::g1_point_add(p1_bytes, p2_bytes)
}

/// Multiplies a G1 point on the bn128 curve by a scalar via NEAR host function.
#[cfg(feature = "contract")]
pub fn alt_bn128_g1_scalar_multiple(input_bytes: &[u8]) -> Result<[u8; 64], Bn254Error> {
    use aurora_engine_types::U256;
    // Buffer is: [P1(G1_LEN), Scalar(SCALAR_LEN)] -> Total 96 bytes
    const BUFFER_LEN: usize = G1_LEN + SCALAR_LEN;
    // Register ID to store the result
    const REGISTER_ID: u64 = 1;

    const BN128_SCALAR_ORDER: U256 = U256([
        0x43e1f593f0000001,
        0x2833e84879b97091,
        0xb85045b68181585d,
        0x30644e72e131a029,
    ]);
    let mut bytes = [0u8; BUFFER_LEN];

    // 1. Process Point G1 (First 64 bytes)
    // X coordinate (Input 0..32)
    write_reversed_chunk(&mut bytes[0..FQ_LEN], input_bytes, 0);
    // Y coordinate (Input 32..64)
    write_reversed_chunk(&mut bytes[FQ_LEN..G1_LEN], input_bytes, FQ_LEN);

    // 2. Process Scalar (Next 32 bytes)
    // Scalar (Input 64..96)
    write_reversed_chunk(&mut bytes[G1_LEN..], input_bytes, G1_LEN);

    let scalar_slice = &mut bytes[G1_LEN..];
    let scalar = U256::from_little_endian(scalar_slice);
    if scalar >= BN128_SCALAR_ORDER {
        let normalized = scalar % BN128_SCALAR_ORDER;
        scalar_slice.copy_from_slice(&normalized.to_little_endian());
    }

    let value_ptr = bytes.as_ptr() as u64;
    let value_len = bytes.len() as u64;
    // Prepare output buffer
    let mut output = [0u8; G1_LEN];
    // Call the NEAR host function
    unsafe {
        exports::alt_bn128_g1_multiexp(value_len, value_ptr, REGISTER_ID);
        exports::read_register(REGISTER_ID, output.as_ptr() as u64);
    }

    // 3. Output Point G1 (Little -> Big)
    // X coordinate
    output[0..FQ_LEN].reverse();
    // Y coordinate
    output[FQ_LEN..G1_LEN].reverse();

    Ok(output)
}

#[cfg(not(feature = "contract"))]
pub fn alt_bn128_g1_scalar_multiple(input_bytes: &[u8]) -> Result<[u8; 64], Bn254Error> {
    let input = utils::right_pad::<MUL_INPUT_LEN>(input_bytes);

    let point_bytes = &input[..G1_LEN];
    let scalar_bytes = &input[G1_LEN..G1_LEN + SCALAR_LEN];
    utils::g1_point_mul(point_bytes, scalar_bytes)
}

/// Performs a pairing check on a list of G1 and G2 point pairs via NEAR host function.
/// Accepts a byte slice containing a sequence of pairs (G1, G2).
#[cfg(feature = "contract")]
pub fn alt_bn128_pairing(input_bytes: &[u8]) -> Result<bool, Bn254Error> {
    use aurora_engine_types::vec;

    // Empty input implies the product of an empty set, which is the multiplicative identity (1).
    // Therefore, the check passes.
    if input_bytes.is_empty() {
        return Ok(true);
    }

    // Validate input length
    if input_bytes.len() % PAIR_ELEMENT_LEN != 0 {
        return Err(Bn254Error::InvalidPairLength);
    }

    let len = input_bytes.len();
    let mut bytes = vec![0u8; len];

    // Iterating over input and output chunks simultaneously (Zip).
    // This allows the compiler to elide bounds checks because slice sizes match.
    for (src, dst) in input_bytes
        .chunks_exact(PAIR_ELEMENT_LEN)
        .zip(bytes.chunks_exact_mut(PAIR_ELEMENT_LEN))
    {
        // --- Process G1 (2 * 32 bytes) ---

        // P1.X (0..32)
        dst[0..FQ_LEN].copy_from_slice(&src[0..FQ_LEN]);
        dst[0..FQ_LEN].reverse();

        // P1.Y (32..64)
        dst[FQ_LEN..FQ_LEN * 2].copy_from_slice(&src[FQ_LEN..FQ_LEN * 2]);
        dst[FQ_LEN..FQ_LEN * 2].reverse();

        // --- Process G2 (2 * 64 bytes) ---
        // Note: Reversing the full 64 bytes of Fq2 automatically handles
        // both Endianness swap AND Coefficient swap (c0 <-> c1) required for NEAR format.

        // P2.X (64..128)
        const G2_X_START: usize = FQ_LEN * 2;
        const G2_X_END: usize = G2_X_START + FQ2_LEN;
        dst[G2_X_START..G2_X_END].copy_from_slice(&src[G2_X_START..G2_X_END]);
        dst[G2_X_START..G2_X_END].reverse();

        // P2.Y (128..192)
        const G2_Y_START: usize = G2_X_END;
        dst[G2_Y_START..].copy_from_slice(&src[G2_Y_START..]);
        dst[G2_Y_START..].reverse();
    }
    let value_ptr = bytes.as_ptr() as u64;
    let value_len = bytes.len() as u64;

    // Call Host Function.
    let result = unsafe { exports::alt_bn128_pairing_check(value_len, value_ptr) };
    Ok(result == 1)
}

#[cfg(not(feature = "contract"))]
pub fn alt_bn128_pairing(input_bytes: &[u8]) -> Result<bool, Bn254Error> {
    // Empty input implies the product of an empty set, which is the multiplicative identity (1).
    // Therefore, the check passes.
    if input_bytes.is_empty() {
        return Ok(true);
    }

    // Validate input length
    if input_bytes.len() % PAIR_ELEMENT_LEN != 0 {
        return Err(Bn254Error::InvalidPairLength);
    }

    let elements = input_bytes.len() / PAIR_ELEMENT_LEN;

    let mut points = Vec::with_capacity(elements);

    for idx in 0..elements {
        // Offset to the start of the pairing element at index `idx` in the byte slice
        let start = idx * PAIR_ELEMENT_LEN;
        let g1_start = start;
        // Offset to the start of the G2 element in the pairing element
        // This is where G1 ends.
        let g2_start = start + G1_LEN;

        // Get G1 and G2 points from the input
        let encoded_g1_element = &input_bytes[g1_start..g2_start];
        let encoded_g2_element = &input_bytes[g2_start..g2_start + G2_LEN];
        points.push((encoded_g1_element, encoded_g2_element));
    }

    utils::pairing_check(&points)
}

// Helper: copy available bytes from input to dest, then reverse (BE -> LE)
// Works for both FQ elements (coordinates) and Scalar, since both are 32 bytes.
#[cfg(feature = "contract")]
#[inline]
fn write_reversed_chunk(dest: &mut [u8], input: &[u8], offset: usize) {
    if let Some(src) = input.get(offset..) {
        let len = src.len().min(FQ_LEN); // FQ_LEN == SCALAR_LEN == 32
        dest[..len].copy_from_slice(&src[..len]);
    }
    dest.reverse();
}

/// Helper for direct reverse writing.
/// Marked `inline` to dissolve into the loop for minimal gas overhead.
#[cfg(feature = "contract")]
#[inline]
unsafe fn write_reversed_raw(src: &[u8], dst: *mut u8) {
    let len = src.len();
    for i in 0..len {
        // Read from end -> Write to start
        // SAFETY: Caller guarantees src and dst are valid and non-overlapping.
        // Loop bounds guarantee checking is valid.
        let byte = *src.get_unchecked(len - 1 - i);
        *dst.add(i) = byte;
    }
}

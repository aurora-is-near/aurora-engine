//! # BLS12-381
//!
//! Represents [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537)

use blst::{
    blst_bendian_from_fp, blst_final_exp, blst_fp, blst_fp12, blst_fp12_is_one, blst_fp12_mul,
    blst_fp_from_bendian, blst_map_to_g1, blst_map_to_g2, blst_miller_loop, blst_p1,
    blst_p1_add_or_double_affine, blst_p1_affine, blst_p1_from_affine, blst_p1_to_affine, blst_p2,
    blst_p2_add_or_double_affine, blst_p2_affine, blst_p2_from_affine, blst_p2_to_affine,
    blst_scalar, blst_scalar_from_bendian, p1_affines, p2_affines,
};

pub mod g1;
pub mod g2;

/// Length of each of the elements in a g1 operation input.
const G1_INPUT_ITEM_LENGTH: usize = 128;
/// Length of each of the elements in a g2 operation input.
const G2_INPUT_ITEM_LENGTH: usize = 256;
/// Input length of `g1_mul` operation.
const G1_MUL_INPUT_LENGTH: usize = 160;
/// Input length of `g2_mul` operation.
const G2_INPUT_LENGTH: usize = 288;
/// Finite field element input length.
const FP_LENGTH: usize = 48;
/// Finite field element padded input length.
const PADDED_FP_LENGTH: usize = 64;
/// Quadratic extension of finite field element input length.
const PADDED_FP2_LENGTH: usize = 128;
/// Input elements padding length.
const PADDING_LENGTH: usize = 16;
/// Scalar length.
const SCALAR_LENGTH: usize = 32;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum Bls12381Error {
    Padding,
    UsizeConversion,
    G1InputLength,
    ElementNotInG1,
    ElementNotInG2,
    InvalidFpValue,
    ScalarLength,
}

impl AsRef<&'static str> for Bls12381Error {
    fn as_ref(&self) -> &&'static str {
        match self {
            Self::Padding => &"ERR_BLS12_PADDING",
            Self::UsizeConversion => &"ERR_BLS12_USIZE_CONVERSION",
            Self::G1InputLength => &"ERR_BLS12_G1_INPUT_LENGTH",
            Self::ElementNotInG1 => &"ERR_BLS12_ELEMENT_NOT_IN_G1",
            Self::ElementNotInG2 => &"ERR_BLS12_ELEMENT_NOT_IN_G2",
            Self::InvalidFpValue => &"ERR_BLS12_FP_VALUE",
            Self::ScalarLength => &"ERR_BLS12_SCALAR_LENGTH",
        }
    }
}

pub fn g1_add(input: &[u8]) -> Result<Vec<u8>, Bls12381Error> {
    // NB: There is no subgroup check for the G1 addition precompile.
    //
    // We set the subgroup checks here to `false`
    let a_aff = &g1::extract_g1_input(&input[..G1_INPUT_ITEM_LENGTH], false)?;
    let b_aff = &g1::extract_g1_input(&input[G1_INPUT_ITEM_LENGTH..], false)?;

    let mut b = blst_p1::default();
    // SAFETY: b and b_aff are blst values.
    unsafe { blst_p1_from_affine(&mut b, b_aff) };

    let mut p = blst_p1::default();
    // SAFETY: p, b and a_aff are blst values.
    unsafe { blst_p1_add_or_double_affine(&mut p, &b, a_aff) };

    let mut p_aff = blst_p1_affine::default();
    // SAFETY: p_aff and p are blst values.
    unsafe { blst_p1_to_affine(&mut p_aff, &p) };

    Ok(g1::encode_g1_point(&p_aff))
}

pub fn g1_msm(input: &[u8]) -> Result<Vec<u8>, Bls12381Error> {
    let k = input.len() / G1_MUL_INPUT_LENGTH;
    let mut g1_points: Vec<blst_p1> = Vec::with_capacity(k);
    let mut scalars: Vec<u8> = Vec::with_capacity(k * SCALAR_LENGTH);
    for i in 0..k {
        let slice = &input[i * G1_MUL_INPUT_LENGTH..i * G1_MUL_INPUT_LENGTH + G1_INPUT_ITEM_LENGTH];

        // BLST batch API for p1_affines blows up when you pass it a point at infinity, so we must
        // filter points at infinity (and their corresponding scalars) from the input.
        if slice.iter().all(|i| *i == 0) {
            continue;
        }

        // NB: Scalar multiplications, MSMs and pairings MUST perform a subgroup check.
        //
        // So we set the subgroup_check flag to `true`
        let p0_aff = &g1::extract_g1_input(slice, true)?;

        let mut p0 = blst_p1::default();
        // SAFETY: p0 and p0_aff are blst values.
        unsafe { blst_p1_from_affine(&mut p0, p0_aff) };
        g1_points.push(p0);

        scalars.extend_from_slice(
            &extract_scalar_input(
                &input[i * G1_MUL_INPUT_LENGTH + G1_INPUT_ITEM_LENGTH
                    ..i * G1_MUL_INPUT_LENGTH + G1_INPUT_ITEM_LENGTH + SCALAR_LENGTH],
            )?
            .b,
        );
    }

    // return infinity point if all points are infinity
    if g1_points.is_empty() {
        return Ok([0; 128].into());
    }

    let points = p1_affines::from(&g1_points);
    let multiexp = points.mult(&scalars, NBITS);

    let mut multiexp_aff = blst_p1_affine::default();
    // SAFETY: multiexp_aff and multiexp are blst values.
    unsafe { blst_p1_to_affine(&mut multiexp_aff, &multiexp) };

    Ok(g1::encode_g1_point(&multiexp_aff))
}

pub fn g2_add(input: &[u8]) -> Result<Vec<u8>, Bls12381Error> {
    // NB: There is no subgroup check for the G2 addition precompile.
    //
    // So we set the subgroup checks here to `false`
    let a_aff = &g2::extract_g2_input(&input[..G2_INPUT_ITEM_LENGTH], false)?;
    let b_aff = &g2::extract_g2_input(&input[G2_INPUT_ITEM_LENGTH..], false)?;

    let mut b = blst_p2::default();
    // SAFETY: b and b_aff are blst values.
    unsafe { blst_p2_from_affine(&mut b, b_aff) };

    let mut p = blst_p2::default();
    // SAFETY: p, b and a_aff are blst values.
    unsafe { blst_p2_add_or_double_affine(&mut p, &b, a_aff) };

    let mut p_aff = blst_p2_affine::default();
    // SAFETY: p_aff and p are blst values.
    unsafe { blst_p2_to_affine(&mut p_aff, &p) };

    Ok(g2::encode_g2_point(&p_aff))
}

pub fn g2_msm(input: &[u8]) -> Result<Vec<u8>, Bls12381Error> {
    let k = input.len() / G2_INPUT_LENGTH;
    let mut g2_points: Vec<blst_p2> = Vec::with_capacity(k);
    let mut scalars: Vec<u8> = Vec::with_capacity(k * SCALAR_LENGTH);
    for i in 0..k {
        let slice = &input[i * G2_INPUT_LENGTH..i * G2_INPUT_LENGTH + G2_INPUT_ITEM_LENGTH];
        // BLST batch API for p2_affines blows up when you pass it a point at infinity, so we must
        // filter points at infinity (and their corresponding scalars) from the input.
        if slice.iter().all(|i| *i == 0) {
            continue;
        }

        // NB: Scalar multiplications, MSMs and pairings MUST perform a subgroup check.
        //
        // So we set the subgroup_check flag to `true`
        let p0_aff = &g2::extract_g2_input(slice, true)?;

        let mut p0 = blst_p2::default();
        // SAFETY: p0 and p0_aff are blst values.
        unsafe { blst_p2_from_affine(&mut p0, p0_aff) };

        g2_points.push(p0);

        scalars.extend_from_slice(
            &extract_scalar_input(
                &input[i * G2_INPUT_LENGTH + G2_INPUT_ITEM_LENGTH
                    ..i * G2_INPUT_LENGTH + G2_INPUT_ITEM_LENGTH + SCALAR_LENGTH],
            )?
            .b,
        );
    }

    // return infinity point if all points are infinity
    if g2_points.is_empty() {
        return Ok([0; 256].into());
    }

    let points = p2_affines::from(&g2_points);
    let multiexp = points.mult(&scalars, NBITS);

    let mut multiexp_aff = blst_p2_affine::default();
    // SAFETY: multiexp_aff and multiexp are blst values.
    unsafe { blst_p2_to_affine(&mut multiexp_aff, &multiexp) };

    Ok(g2::encode_g2_point(&multiexp_aff))
}

pub fn map_fp2_to_g12(input: &[u8]) -> Result<Vec<u8>, Bls12381Error> {
    let input_p0_x = remove_padding(&input[..PADDED_FP_LENGTH])?;
    let input_p0_y = remove_padding(&input[PADDED_FP_LENGTH..PADDED_FP2_LENGTH])?;
    let fp2 = g2::check_canonical_fp2(input_p0_x, input_p0_y)?;

    let mut p = blst_p2::default();
    // SAFETY: p and fp2 are blst values.
    // third argument is unused if null.
    unsafe { blst_map_to_g2(&mut p, &fp2, core::ptr::null()) };

    let mut p_aff = blst_p2_affine::default();
    // SAFETY: p_aff and p are blst values.
    unsafe { blst_p2_to_affine(&mut p_aff, &p) };

    Ok(g2::encode_g2_point(&p_aff))
}

pub fn map_fp_to_g1(input: &[u8]) -> Result<Vec<u8>, Bls12381Error> {
    let input_p0 = remove_padding(input)?;
    let fp = fp_from_bendian(input_p0)?;

    let mut p = blst_p1::default();
    // SAFETY: p and fp are blst values.
    // third argument is unused if null.
    unsafe { blst_map_to_g1(&mut p, &fp, core::ptr::null()) };

    let mut p_aff = blst_p1_affine::default();
    // SAFETY: p_aff and p are blst values.
    unsafe { blst_p1_to_affine(&mut p_aff, &p) };

    Ok(g1::encode_g1_point(&p_aff))
}

pub fn pairing_check(input: &[u8]) -> Result<Vec<u8>, Bls12381Error> {
    let k = input.len() / PAIRING_INPUT_LENGTH;
    // Accumulator for the fp12 multiplications of the miller loops.
    let mut acc = blst_fp12::default();
    for i in 0..k {
        // NB: Scalar multiplications, MSMs and pairings MUST perform a subgroup check.
        //
        // So we set the subgroup_check flag to `true`
        let p1_aff = &g1::extract_g1_input(
            &input[i * PAIRING_INPUT_LENGTH..i * PAIRING_INPUT_LENGTH + G1_INPUT_ITEM_LENGTH],
            true,
        )?;

        // NB: Scalar multiplications, MSMs and pairings MUST perform a subgroup check.
        //
        // So we set the subgroup_check flag to `true`
        let p2_aff = &g2::extract_g2_input(
            &input[i * PAIRING_INPUT_LENGTH + G1_INPUT_ITEM_LENGTH
                ..i * PAIRING_INPUT_LENGTH + G1_INPUT_ITEM_LENGTH + G2_INPUT_ITEM_LENGTH],
            true,
        )?;

        if i > 0 {
            // After the first slice (i>0) we use cur_ml to store the current
            // miller loop and accumulate with the previous results using a fp12
            // multiplication.
            let mut cur_ml = blst_fp12::default();
            let mut res = blst_fp12::default();
            // SAFETY: res, acc, cur_ml, p1_aff and p2_aff are blst values.
            unsafe {
                blst_miller_loop(&mut cur_ml, p2_aff, p1_aff);
                blst_fp12_mul(&mut res, &acc, &cur_ml);
            }
            acc = res;
        } else {
            // On the first slice (i==0) there is no previous results and no need
            // to accumulate.
            // SAFETY: acc, p1_aff and p2_aff are blst values.
            unsafe {
                blst_miller_loop(&mut acc, p2_aff, p1_aff);
            }
        }
    }

    // SAFETY: ret and acc are blst values.
    let mut ret = blst_fp12::default();
    unsafe {
        blst_final_exp(&mut ret, &acc);
    }

    let mut result: u8 = 0;
    // SAFETY: ret is a blst value.
    unsafe {
        if blst_fp12_is_one(&ret) {
            result = 1;
        }
    }
    let mut output = [0u8; 32];
    output[31] = result;
    Ok(output.into())
}

/// Input length of pairing operation.
const PAIRING_INPUT_LENGTH: usize = 384;
/// Number of bits used in the BLS12-381 curve finite field elements.
const NBITS: usize = 256;
/// Big-endian non-Montgomery form.
const MODULUS_REPR: [u8; 48] = [
    0x1a, 0x01, 0x11, 0xea, 0x39, 0x7f, 0xe6, 0x9a, 0x4b, 0x1b, 0xa7, 0xb6, 0x43, 0x4b, 0xac, 0xd7,
    0x64, 0x77, 0x4b, 0x84, 0xf3, 0x85, 0x12, 0xbf, 0x67, 0x30, 0xd2, 0xa0, 0xf6, 0xb0, 0xf6, 0x24,
    0x1e, 0xab, 0xff, 0xfe, 0xb1, 0x53, 0xff, 0xff, 0xb9, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xaa, 0xab,
];

/// BLS Encodes a single finite field element into byte slice with padding.
fn fp_to_bytes(out: &mut [u8], input: *const blst_fp) {
    if out.len() != PADDED_FP_LENGTH {
        return;
    }
    let (padding, rest) = out.split_at_mut(PADDING_LENGTH);
    padding.fill(0);
    unsafe { blst_bendian_from_fp(rest.as_mut_ptr(), input) };
}

/// Checks if the input is a valid big-endian representation of a field element.
fn is_valid_be(input: &[u8; 48]) -> bool {
    for (i, modul) in input.iter().zip(MODULUS_REPR.iter()) {
        match i.cmp(modul) {
            core::cmp::Ordering::Greater => return false,
            core::cmp::Ordering::Less => return true,
            core::cmp::Ordering::Equal => continue,
        }
    }
    // false if matching the modulus
    false
}

/// Checks whether or not the input represents a canonical field element, returning the field
/// element if successful.
fn fp_from_bendian(input: &[u8; 48]) -> Result<blst_fp, Bls12381Error> {
    if !is_valid_be(input) {
        return Err(Bls12381Error::ElementNotInG2);
    }
    let mut fp = blst_fp::default();
    // SAFETY: input has fixed length, and fp is a blst value.
    unsafe {
        // This performs the check for canonical field elements
        blst_fp_from_bendian(&mut fp, input.as_ptr());
    }
    Ok(fp)
}

/// Extracts a scalar from a 32 byte slice representation, decoding the input as a big endian
/// unsigned integer. If the input is not exactly 32 bytes long, an error is returned.
///
/// From [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537):
/// * A scalar for the multiplication operation is encoded as 32 bytes by performing `BigEndian`
///   encoding of the corresponding (unsigned) integer.
///
/// We do not check that the scalar is a canonical Fr element, because the EIP specifies:
/// * The corresponding integer is not required to be less than or equal than main subgroup order
///   `q`.
fn extract_scalar_input(input: &[u8]) -> Result<blst_scalar, Bls12381Error> {
    if input.len() != SCALAR_LENGTH {
        return Err(crate::bls12_381::Bls12381Error::ScalarLength);
    }

    let mut out = blst_scalar::default();
    // SAFETY: input length is checked previously, out is a blst value.
    unsafe {
        // NOTE: we do not use `blst_scalar_fr_check` here because, from EIP-2537:
        //
        // * The corresponding integer is not required to be less than or equal than main subgroup
        // order `q`.
        blst_scalar_from_bendian(&mut out, input.as_ptr());
    };

    Ok(out)
}

/// Removes zeros with which the precompile inputs are left padded to 64 bytes.
fn remove_padding(input: &[u8]) -> Result<&[u8; FP_LENGTH], Bls12381Error> {
    if input.len() != PADDED_FP_LENGTH {
        return Err(Bls12381Error::Padding);
    }
    // Check is prefix contains only zero elements. As it's known size
    // 16 bytes for efficiency we validate it via slice with zero elements
    if input[..PADDING_LENGTH] != [0u8; PADDING_LENGTH] {
        return Err(Bls12381Error::Padding);
    }
    // SAFETY: we checked PADDED_FP_LENGTH
    input[PADDING_LENGTH..]
        .try_into()
        .map_err(|_| Bls12381Error::Padding)
}

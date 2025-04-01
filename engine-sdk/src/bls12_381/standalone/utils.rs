use crate::bls12_381::standalone::SCALAR_LENGTH;
use crate::bls12_381::{Bls12381Error, FP_LENGTH, PADDED_FP_LENGTH, PADDING_LENGTH};
use blst::{
    blst_bendian_from_fp, blst_fp, blst_fp_from_bendian, blst_scalar, blst_scalar_from_bendian,
};

/// Big-endian non-Montgomery form.
const MODULUS_REPR: [u8; 48] = [
    0x1a, 0x01, 0x11, 0xea, 0x39, 0x7f, 0xe6, 0x9a, 0x4b, 0x1b, 0xa7, 0xb6, 0x43, 0x4b, 0xac, 0xd7,
    0x64, 0x77, 0x4b, 0x84, 0xf3, 0x85, 0x12, 0xbf, 0x67, 0x30, 0xd2, 0xa0, 0xf6, 0xb0, 0xf6, 0x24,
    0x1e, 0xab, 0xff, 0xfe, 0xb1, 0x53, 0xff, 0xff, 0xb9, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xaa, 0xab,
];

/// BLS Encodes a single finite field element into byte slice with padding.
pub fn fp_to_bytes(out: &mut [u8], input: *const blst_fp) {
    if out.len() != PADDED_FP_LENGTH {
        return;
    }
    let (padding, rest) = out.split_at_mut(PADDING_LENGTH);
    padding.fill(0);
    unsafe { blst_bendian_from_fp(rest.as_mut_ptr(), input) };
}

/// Checks if the input is a valid big-endian representation of a field element.
pub fn is_valid_be(input: &[u8; 48]) -> bool {
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
pub fn fp_from_bendian(input: &[u8; 48]) -> Result<blst_fp, Bls12381Error> {
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
pub fn extract_scalar_input(input: &[u8]) -> Result<blst_scalar, Bls12381Error> {
    if input.len() != SCALAR_LENGTH {
        return Err(Bls12381Error::ScalarLength);
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
pub fn remove_padding(input: &[u8]) -> Result<&[u8; FP_LENGTH], Bls12381Error> {
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

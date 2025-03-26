use super::{
    Bls12381Error, FP_LENGTH, G1_INPUT_ITEM_LENGTH, G1_MUL_INPUT_LENGTH, G2_INPUT_ITEM_LENGTH,
    G2_MUL_INPUT_LENGTH, PADDED_FP_LENGTH, PADDING_LENGTH, PAIRING_INPUT_LENGTH,
};
use crate::prelude::{vec, Vec};

pub mod exports;

// Scalar length.
const SCALAR_LENGTH: usize = 32;

#[must_use]
fn padding_g1_result(output: &[u8; 2 * FP_LENGTH]) -> Vec<u8> {
    let mut result = vec![0u8; 2 * PADDED_FP_LENGTH];
    if output[0] == 0x40 && output[1..] == [0u8; 2 * FP_LENGTH - 1] {
        return result;
    }
    result[PADDING_LENGTH..PADDED_FP_LENGTH].copy_from_slice(&output[..FP_LENGTH]);
    result[PADDING_LENGTH + PADDED_FP_LENGTH..2 * PADDED_FP_LENGTH]
        .copy_from_slice(&output[FP_LENGTH..]);
    result
}

#[must_use]
fn padding_g2_result(output: &[u8; 4 * FP_LENGTH]) -> Vec<u8> {
    let mut result = vec![0u8; 4 * PADDED_FP_LENGTH];
    if output[0] == 0x40 && output[1..] == [0u8; 4 * FP_LENGTH - 1] {
        return result;
    }
    result[PADDING_LENGTH..PADDED_FP_LENGTH].copy_from_slice(&output[FP_LENGTH..2 * FP_LENGTH]);
    result[PADDING_LENGTH + PADDED_FP_LENGTH..2 * PADDED_FP_LENGTH]
        .copy_from_slice(&output[..FP_LENGTH]);
    result[PADDING_LENGTH + 2 * PADDED_FP_LENGTH..3 * PADDED_FP_LENGTH]
        .copy_from_slice(&output[3 * FP_LENGTH..]);
    result[PADDING_LENGTH + 3 * PADDED_FP_LENGTH..4 * PADDED_FP_LENGTH]
        .copy_from_slice(&output[2 * FP_LENGTH..3 * FP_LENGTH]);
    result
}

fn extract_g1(input: &[u8]) -> Result<(&[u8; FP_LENGTH], &[u8; FP_LENGTH]), Bls12381Error> {
    let p_x = remove_padding(&input[..PADDED_FP_LENGTH])?;
    let p_y = remove_padding(&input[PADDED_FP_LENGTH..G1_INPUT_ITEM_LENGTH])?;

    Ok((p_x, p_y))
}

fn extract_g2(input: &[u8]) -> Result<([u8; 2 * FP_LENGTH], [u8; 2 * FP_LENGTH]), Bls12381Error> {
    let p0_last = remove_padding(&input[..PADDED_FP_LENGTH])?;
    let p0_first = remove_padding(&input[PADDED_FP_LENGTH..2 * PADDED_FP_LENGTH])?;
    let p1_last = remove_padding(&input[2 * PADDED_FP_LENGTH..3 * PADDED_FP_LENGTH])?;
    let p1_first = remove_padding(&input[3 * PADDED_FP_LENGTH..4 * PADDED_FP_LENGTH])?;

    let mut p_x = [0u8; 2 * FP_LENGTH];
    p_x[0..FP_LENGTH].copy_from_slice(p0_first);
    p_x[FP_LENGTH..].copy_from_slice(p0_last);

    let mut p_y = [0u8; 2 * FP_LENGTH];
    p_y[0..FP_LENGTH].copy_from_slice(p1_first);
    p_y[FP_LENGTH..].copy_from_slice(p1_last);

    Ok((p_x, p_y))
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

#[allow(clippy::range_plus_one)]
pub fn g1_add(input: &[u8]) -> Result<Vec<u8>, Bls12381Error> {
    let (p0_x, p0_y) = extract_g1(&input[..G1_INPUT_ITEM_LENGTH])?;
    let (p1_x, p1_y) = extract_g1(&input[G1_INPUT_ITEM_LENGTH..])?;

    let mut g1_input = [0u8; 4 * FP_LENGTH + 2];

    if input[..G1_INPUT_ITEM_LENGTH] == [0; G1_INPUT_ITEM_LENGTH] {
        g1_input[1] |= 0x40;
    } else {
        g1_input[1..1 + FP_LENGTH].copy_from_slice(p0_x);
        g1_input[1 + FP_LENGTH..1 + 2 * FP_LENGTH].copy_from_slice(p0_y);
    }

    if input[G1_INPUT_ITEM_LENGTH..] == [0; G1_INPUT_ITEM_LENGTH] {
        g1_input[2 + 2 * FP_LENGTH] |= 0x40;
    } else {
        g1_input[2 + 2 * FP_LENGTH..2 + 3 * FP_LENGTH].copy_from_slice(p1_x);
        g1_input[2 + 3 * FP_LENGTH..2 + 4 * FP_LENGTH].copy_from_slice(p1_y);
    }

    let output = exports::bls12381_p1_sum(&g1_input[..]);
    Ok(padding_g1_result(&output))
}

pub fn g1_msm(input: &[u8]) -> Result<Vec<u8>, Bls12381Error> {
    let k = input.len() / G1_MUL_INPUT_LENGTH;
    let mut g1_input = vec![0u8; k * (2 * FP_LENGTH + SCALAR_LENGTH)];
    for i in 0..k {
        let (p0_x, p0_y) = extract_g1(
            &input[i * G1_MUL_INPUT_LENGTH..i * G1_MUL_INPUT_LENGTH + G1_INPUT_ITEM_LENGTH],
        )?;
        // Data offset for the points
        let offset = i * (2 * FP_LENGTH + SCALAR_LENGTH);
        // Check is p0 zero coordinate
        if input[i * G1_MUL_INPUT_LENGTH..i * G1_MUL_INPUT_LENGTH + G1_INPUT_ITEM_LENGTH]
            == [0; G1_INPUT_ITEM_LENGTH]
        {
            g1_input[offset] = 0x40;
        } else {
            g1_input[offset..offset + FP_LENGTH].copy_from_slice(p0_x);
            g1_input[offset + FP_LENGTH..offset + 2 * FP_LENGTH].copy_from_slice(p0_y);
        }
        // Set scalar
        let g1_range = offset + 2 * FP_LENGTH..offset + 2 * FP_LENGTH + SCALAR_LENGTH;
        let scalar =
            &input[(i + 1) * G1_MUL_INPUT_LENGTH - SCALAR_LENGTH..(i + 1) * G1_MUL_INPUT_LENGTH];
        g1_input[g1_range.clone()].copy_from_slice(scalar);
        g1_input[g1_range].reverse();
    }

    let output = exports::bls12381_g1_multiexp(&g1_input[..]);
    Ok(padding_g1_result(&output))
}

#[allow(clippy::range_plus_one)]
pub fn g2_add(input: &[u8]) -> Result<Vec<u8>, Bls12381Error> {
    let (p0_x, p0_y) = extract_g2(&input[..G2_INPUT_ITEM_LENGTH])?;
    let (p1_x, p1_y) = extract_g2(&input[G2_INPUT_ITEM_LENGTH..])?;

    let mut g2_input = [0u8; 8 * FP_LENGTH + 2];

    // Check zero input
    if input[..G2_INPUT_ITEM_LENGTH] == [0; G2_INPUT_ITEM_LENGTH] {
        g2_input[1] |= 0x40;
    } else {
        g2_input[1..1 + 2 * FP_LENGTH].copy_from_slice(&p0_x);
        g2_input[1 + 2 * FP_LENGTH..1 + 4 * FP_LENGTH].copy_from_slice(&p0_y);
    }

    if input[G2_INPUT_ITEM_LENGTH..] == [0; G2_INPUT_ITEM_LENGTH] {
        g2_input[2 + 4 * FP_LENGTH] |= 0x40;
    } else {
        g2_input[2 + 4 * FP_LENGTH..2 + 6 * FP_LENGTH].copy_from_slice(&p1_x);
        g2_input[2 + 6 * FP_LENGTH..2 + 8 * FP_LENGTH].copy_from_slice(&p1_y);
    }

    let output = exports::bls12381_p2_sum(&g2_input[..]);
    Ok(padding_g2_result(&output))
}

pub fn g2_msm(input: &[u8]) -> Result<Vec<u8>, Bls12381Error> {
    let k = input.len() / G2_MUL_INPUT_LENGTH;
    let mut g2_input = vec![0u8; k * (4 * FP_LENGTH + SCALAR_LENGTH)];
    for i in 0..k {
        let (p0_x, p0_y) = extract_g2(
            &input[i * G2_MUL_INPUT_LENGTH..i * G2_MUL_INPUT_LENGTH + G2_INPUT_ITEM_LENGTH],
        )?;

        // Data offset for the points
        let offset = i * (4 * FP_LENGTH + SCALAR_LENGTH);
        // Check is p0 zero coordinate
        if input[i * G2_MUL_INPUT_LENGTH..i * G2_MUL_INPUT_LENGTH + G2_INPUT_ITEM_LENGTH]
            == [0; G2_INPUT_ITEM_LENGTH]
        {
            g2_input[offset] = 0x40;
        } else {
            g2_input[offset..offset + 2 * FP_LENGTH].copy_from_slice(&p0_x);
            g2_input[offset + 2 * FP_LENGTH..offset + 4 * FP_LENGTH].copy_from_slice(&p0_y);
        }
        // Set scalar
        let g2_range = offset + 4 * FP_LENGTH..offset + 4 * FP_LENGTH + SCALAR_LENGTH;
        let scalar =
            &input[(i + 1) * G2_MUL_INPUT_LENGTH - SCALAR_LENGTH..(i + 1) * G2_MUL_INPUT_LENGTH];
        g2_input[g2_range.clone()].copy_from_slice(scalar);
        g2_input[g2_range].reverse();
    }

    let output = exports::bls12381_g2_multiexp(&g2_input[..]);
    Ok(padding_g2_result(&output))
}

pub fn map_fp_to_g1(input: &[u8]) -> Result<Vec<u8>, Bls12381Error> {
    let p = remove_padding(input)?;
    let output = exports::bls12381_map_fp_to_g1(&p[..]);
    Ok(padding_g1_result(&output))
}

pub fn map_fp2_to_g2(input: &[u8]) -> Result<Vec<u8>, Bls12381Error> {
    let mut p = [0; 2 * FP_LENGTH];
    let p1 = remove_padding(&input[..PADDED_FP_LENGTH])?;
    let p2 = remove_padding(&input[PADDED_FP_LENGTH..])?;
    p[..FP_LENGTH].copy_from_slice(p2);
    p[FP_LENGTH..].copy_from_slice(p1);

    let output = exports::bls12381_map_fp2_to_g2(&p[..]);
    Ok(padding_g2_result(&output))
}

pub fn pairing_check(input: &[u8]) -> Result<Vec<u8>, Bls12381Error> {
    let k = input.len() / PAIRING_INPUT_LENGTH;
    let mut g_input = vec![0u8; k * (6 * FP_LENGTH)];
    for i in 0..k {
        let offset = i * (G1_INPUT_ITEM_LENGTH + G2_INPUT_ITEM_LENGTH);
        let data_offset = i * 6 * FP_LENGTH;
        let (p0_x, p0_y) = extract_g1(&input[offset..offset + G1_INPUT_ITEM_LENGTH])?;
        let (p1_x, p1_y) = extract_g2(
            &input[offset + G1_INPUT_ITEM_LENGTH
                ..offset + G1_INPUT_ITEM_LENGTH + G2_INPUT_ITEM_LENGTH],
        )?;

        if input[offset..offset + G1_INPUT_ITEM_LENGTH] == [0; G1_INPUT_ITEM_LENGTH] {
            g_input[data_offset] |= 0x40;
        } else {
            g_input[data_offset..data_offset + FP_LENGTH].copy_from_slice(p0_x);
            g_input[data_offset + FP_LENGTH..data_offset + 2 * FP_LENGTH].copy_from_slice(p0_y);
        }

        if input
            [offset + G1_INPUT_ITEM_LENGTH..offset + G1_INPUT_ITEM_LENGTH + G2_INPUT_ITEM_LENGTH]
            == [0; G2_INPUT_ITEM_LENGTH]
        {
            g_input[data_offset + 2 * FP_LENGTH] |= 0x40;
        } else {
            g_input[data_offset + 2 * FP_LENGTH..data_offset + 4 * FP_LENGTH]
                .copy_from_slice(&p1_x);
            g_input[data_offset + 4 * FP_LENGTH..data_offset + 6 * FP_LENGTH]
                .copy_from_slice(&p1_y);
        }
    }

    let output = exports::bls12381_pairing_check(&g_input[..]);
    let output = if output == 2 {
        vec![0; 32]
    } else {
        let mut res = vec![0; 31];
        res.push(1);
        res
    };
    Ok(output)
}

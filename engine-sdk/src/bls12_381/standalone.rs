//! # BLS12-381
//!
//! Represents [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537)

use super::{
    remove_padding, Bls12381Error, G1_INPUT_ITEM_LENGTH, G2_INPUT_ITEM_LENGTH, PADDED_FP2_LENGTH,
    PADDED_FP_LENGTH,
};
use crate::prelude::Vec;
use blst::{
    blst_final_exp, blst_fp12, blst_fp12_is_one, blst_fp12_mul, blst_map_to_g1, blst_map_to_g2,
    blst_miller_loop, blst_p1, blst_p1_add_or_double_affine, blst_p1_affine, blst_p1_from_affine,
    blst_p1_to_affine, blst_p2, blst_p2_add_or_double_affine, blst_p2_affine, blst_p2_from_affine,
    blst_p2_to_affine, p1_affines, p2_affines,
};

mod g1;
mod g2;
mod utils;

/// Input length of `g1_mul` operation.
const G1_MUL_INPUT_LENGTH: usize = 160;
/// Input length of `g2_mul` operation.
const G2_INPUT_LENGTH: usize = 288;
/// Scalar length.
const SCALAR_LENGTH: usize = 32;
/// Input length of pairing operation.
const PAIRING_INPUT_LENGTH: usize = 384;
/// Number of bits used in the BLS12-381 curve finite field elements.
const NBITS: usize = 256;

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
            &utils::extract_scalar_input(
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
            &utils::extract_scalar_input(
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

pub fn map_fp2_to_g2(input: &[u8]) -> Result<Vec<u8>, Bls12381Error> {
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
    let fp = utils::fp_from_bendian(input_p0)?;

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

//! # BLS12-381
//!
//! Represents [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537)
use crate::prelude::{vec, Borrowed, Vec};
use evm::ExitError;

mod g1_add;
mod g1_msm;
mod g2_add;
mod g2_msm;
mod map_fp2_to_g2;
mod map_fp_to_g1;
mod pairing_check;
#[cfg(not(feature = "contract"))]
mod standalone;

pub use g1_add::BlsG1Add;
pub use g1_msm::BlsG1Msm;
pub use g2_add::BlsG2Add;
pub use g2_msm::BlsG2Msm;
pub use map_fp2_to_g2::BlsMapFp2ToG2;
pub use map_fp_to_g1::BlsMapFpToG1;
pub use pairing_check::BlsPairingCheck;

/// Length of each of the elements in a g1 operation input.
const G1_INPUT_ITEM_LENGTH: usize = 128;
/// Amount used to calculate the multi-scalar-multiplication discount.
const MSM_MULTIPLIER: u64 = 1000;
/// Finite field element input length.
const FP_LENGTH: usize = 48;
/// Finite field element padded input length.
pub const PADDED_FP_LENGTH: usize = 64;
/// Quadratic extension of finite field element input length.
pub const PADDED_FP2_LENGTH: usize = 128;
/// Input elements padding length.
const PADDING_LENGTH: usize = 16;
/// Scalar length.
const SCALAR_LENGTH: usize = 32;

/// Removes zeros with which the precompile inputs are left padded to 64 bytes.
fn remove_padding(input: &[u8]) -> Result<&[u8; FP_LENGTH], ExitError> {
    if input.len() != PADDED_FP_LENGTH {
        return Err(ExitError::Other(Borrowed("ERR_BLS12_PADDING")));
    }
    // Check is prefix contains only zero elements. As it's known size
    // 16 bytes for efficiency we validate it via slice with zero elements
    if input[..PADDING_LENGTH] != [0u8; PADDING_LENGTH] {
        return Err(ExitError::Other(Borrowed("ERR_BLS12_PADDING")));
    }
    // SAFETY: we checked PADDED_FP_LENGTH
    Ok(unsafe { &*input[PADDING_LENGTH..].as_ptr().cast::<[u8; FP_LENGTH]>() })
}

/// Implements the gas schedule for G1/G2 Multiscalar-multiplication assuming 30
/// MGas/second, see also: <https://eips.ethereum.org/EIPS/eip-2537#g1g2-multiexponentiation>
fn msm_required_gas(
    k: usize,
    discount_table: &[u16],
    multiplication_cost: u64,
) -> Result<u64, ExitError> {
    if k == 0 {
        return Ok(0);
    }

    let index = core::cmp::min(k - 1, discount_table.len() - 1);
    let discount = u64::from(discount_table[index]);

    let k = u64::try_from(k).map_err(crate::utils::err_usize_conv)?;
    Ok((k * discount * multiplication_cost) / MSM_MULTIPLIER)
}

pub fn extract_g1(input: &[u8]) -> Result<(&[u8; FP_LENGTH], &[u8; FP_LENGTH]), ExitError> {
    let p_x = remove_padding(&input[..PADDED_FP_LENGTH])?;
    let p_y = remove_padding(&input[PADDED_FP_LENGTH..G1_INPUT_ITEM_LENGTH])?;

    Ok((p_x, p_y))
}

#[cfg(feature = "contract")]
#[must_use]
pub(super) fn padding_g1_result(output: &[u8; 2 * FP_LENGTH]) -> Vec<u8> {
    let mut result = vec![0u8; 2 * PADDED_FP_LENGTH];
    result[PADDING_LENGTH..PADDED_FP_LENGTH].copy_from_slice(&output[..FP_LENGTH]);
    result[PADDING_LENGTH + PADDED_FP_LENGTH..2 * PADDED_FP_LENGTH]
        .copy_from_slice(&output[FP_LENGTH..]);
    result
}

#[cfg(feature = "contract")]
#[must_use]
pub(super) fn padding_g2_result(output: &[u8; 4 * FP_LENGTH]) -> Vec<u8> {
    let mut result = vec![0u8; 4 * PADDED_FP_LENGTH];
    result[PADDING_LENGTH..PADDED_FP_LENGTH].copy_from_slice(&output[..FP_LENGTH]);
    result[PADDING_LENGTH + PADDED_FP_LENGTH..2 * PADDED_FP_LENGTH]
        .copy_from_slice(&output[FP_LENGTH..2 * FP_LENGTH]);
    result[PADDING_LENGTH + 2 * PADDED_FP_LENGTH..3 * PADDED_FP_LENGTH]
        .copy_from_slice(&output[2 * FP_LENGTH..3 * FP_LENGTH]);
    result[PADDING_LENGTH + 3 * PADDED_FP_LENGTH..4 * PADDED_FP_LENGTH]
        .copy_from_slice(&output[3 * FP_LENGTH..]);
    result
}

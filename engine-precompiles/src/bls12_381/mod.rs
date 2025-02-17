//! # BLS12-381
//!
//! Represents [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537)
use crate::prelude::Borrowed;
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

    let k = u64::try_from(k).map_err(utils::err_usize_conv)?;
    Ok((k * discount * multiplication_cost) / crate::bls12_381::standalone::MSM_MULTIPLIER)
}

#[cfg(feature = "contract")]
pub const G1_TRANSFORMED_INPUT_LENGTH: usize = 194;

#[cfg(feature = "contract")]
pub fn extract_g1(
    input: &[u8],
) -> Result<(&[u8; super::FP_LENGTH], &[u8; super::FP_LENGTH]), ExitError> {
    let p_x = remove_padding(&input[..PADDED_FP_LENGTH])?;
    let p_y = remove_padding(
        &input[PADDED_FP_LENGTH..crate::bls12_381::standalone::g1::G1_INPUT_ITEM_LENGTH],
    )?;

    Ok((p_x, p_y))
}

#[cfg(feature = "contract")]
pub fn transform_input(input: &[u8]) -> Result<(&[u8; G1_TRANSFORMED_INPUT_LENGTH]), ExitError> {
    use super::FP_LENGTH;

    let (p0_x, p0_y) =
        extract_g1(&input[..crate::bls12_381::standalone::g1::G1_INPUT_ITEM_LENGTH])?;
    let (p1_x, p1_y) =
        extract_g1(&input[crate::bls12_381::standalone::g1::G1_INPUT_ITEM_LENGTH..])?;

    let mut g1_input = [0u8; 4 * FP_LENGTH + 2];
    g1_input[0] = 0;
    g1_input[1..1 + FP_LENGTH].copy_from_slice(p0_x);
    g1_input[1 + FP_LENGTH..1 + 2 * FP_LENGTH].copy_from_slice(p0_y);
    g1_input[1 + 2 * FP_LENGTH] = 0;
    g1_input[2 + 2 * FP_LENGTH..2 + 3 * FP_LENGTH].copy_from_slice(p1_x);
    g1_input[2 + 3 * FP_LENGTH..2 + 4 * FP_LENGTH].copy_from_slice(p1_y);
    Ok(&g1_input)
}

//! # BLS12-381
//!
//! Represents [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537)

pub use g1_add::BlsG1Add;
pub use g1_msm::BlsG1Msm;
pub use g2_add::BlsG2Add;
pub use g2_msm::BlsG2Msm;
pub use map_fp2_to_g2::BlsMapFp2ToG2;
pub use map_fp_to_g1::BlsMapFpToG1;
pub use pairing_check::BlsPairingCheck;

mod g1_add;
mod g1_msm;
mod g2_add;
mod g2_msm;
mod map_fp2_to_g2;
mod map_fp_to_g1;
mod pairing_check;

/// Amount used to calculate the multi-scalar-multiplication discount.
const MSM_MULTIPLIER: u64 = 1000;

/// Implements the gas schedule for G1/G2 Multiscalar-multiplication assuming 30
/// MGas/second, see also: <https://eips.ethereum.org/EIPS/eip-2537#g1g2-multiexponentiation>
fn msm_required_gas(
    k: usize,
    discount_table: &[u16],
    multiplication_cost: u64,
) -> Result<u64, aurora_evm::ExitError> {
    if k == 0 {
        return Ok(0);
    }

    let index = core::cmp::min(k - 1, discount_table.len() - 1);
    let discount = u64::from(discount_table[index]);

    let k = u64::try_from(k).map_err(crate::utils::err_usize_conv)?;
    Ok((k * discount * multiplication_cost) / MSM_MULTIPLIER)
}

use super::{extract_scalar_input, g1, msm_required_gas, NBITS, SCALAR_LENGTH};
use crate::prelude::types::{make_address, Address, EthGas};
use crate::prelude::{Borrowed, Vec};
use crate::{EvmPrecompileResult, Precompile, PrecompileOutput};
use blst::{blst_p1, blst_p1_affine, blst_p1_from_affine, blst_p1_to_affine, p1_affines};
use evm::{Context, ExitError};

/// Input length of `g1_mul` operation.
const INPUT_LENGTH: usize = 160;

/// Base gas fee for BLS12-381 `g1_mul` operation.
pub const BASE_GAS_FEE: u64 = 12000;

/// Discounts table for G1 MSM as a vector of pairs `[k, discount]`.
const DISCOUNT_TABLE: [u16; 128] = [
    1000, 949, 848, 797, 764, 750, 738, 728, 719, 712, 705, 698, 692, 687, 682, 677, 673, 669, 665,
    661, 658, 654, 651, 648, 645, 642, 640, 637, 635, 632, 630, 627, 625, 623, 621, 619, 617, 615,
    613, 611, 609, 608, 606, 604, 603, 601, 599, 598, 596, 595, 593, 592, 591, 589, 588, 586, 585,
    584, 582, 581, 580, 579, 577, 576, 575, 574, 573, 572, 570, 569, 568, 567, 566, 565, 564, 563,
    562, 561, 560, 559, 558, 557, 556, 555, 554, 553, 552, 551, 550, 549, 548, 547, 547, 546, 545,
    544, 543, 542, 541, 540, 540, 539, 538, 537, 536, 536, 535, 534, 533, 532, 532, 531, 530, 529,
    528, 528, 527, 526, 525, 525, 524, 523, 522, 522, 521, 520, 520, 519,
];

/// BLS12-382 G1 MSM
pub struct BlsG1Msm;

impl BlsG1Msm {
    pub const ADDRESS: Address = make_address(0, 0xC);
}

impl Precompile for BlsG1Msm {
    fn required_gas(input: &[u8]) -> Result<EthGas, ExitError>
    where
        Self: Sized,
    {
        let k = input.len() / INPUT_LENGTH;
        Ok(EthGas::new(msm_required_gas(
            k,
            &DISCOUNT_TABLE,
            BASE_GAS_FEE,
        )?))
    }

    /// Implements EIP-2537 G1MSM precompile.
    /// G1 multi-scalar-multiplication call expects `160*k` bytes as an input that is interpreted
    /// as byte concatenation of `k` slices each of them being a byte concatenation
    /// of encoding of G1 point (`128` bytes) and encoding of a scalar value (`32`
    /// bytes).
    /// Output is an encoding of multi-scalar-multiplication operation result - single G1
    /// point (`128` bytes).
    /// See also: <https://eips.ethereum.org/EIPS/eip-2537#abi-for-g1-multiexponentiation>
    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        _context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        let input_len = input.len();
        if input_len == 0 || input_len % INPUT_LENGTH != 0 {
            return Err(ExitError::Other(Borrowed("ERR_BLS_G1MSM_INPUT_LEN")));
        }

        let k = input_len / INPUT_LENGTH;
        let cost = Self::required_gas(input)?;
        if let Some(target_gas) = target_gas {
            if cost > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }
        let mut g1_points: Vec<blst_p1> = Vec::with_capacity(k);
        let mut scalars: Vec<u8> = Vec::with_capacity(k * SCALAR_LENGTH);
        for i in 0..k {
            let slice = &input[i * INPUT_LENGTH..i * INPUT_LENGTH + g1::G1_INPUT_ITEM_LENGTH];

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
                    &input[i * INPUT_LENGTH + g1::G1_INPUT_ITEM_LENGTH
                        ..i * INPUT_LENGTH + g1::G1_INPUT_ITEM_LENGTH + SCALAR_LENGTH],
                )?
                .b,
            );
        }

        // return infinity point if all points are infinity
        if g1_points.is_empty() {
            return Ok(PrecompileOutput::without_logs(cost, [0; 128].into()));
        }

        let points = p1_affines::from(&g1_points);
        let multiexp = points.mult(&scalars, NBITS);

        let mut multiexp_aff = blst_p1_affine::default();
        // SAFETY: multiexp_aff and multiexp are blst values.
        unsafe { blst_p1_to_affine(&mut multiexp_aff, &multiexp) };

        let output = g1::encode_g1_point(&multiexp_aff);
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

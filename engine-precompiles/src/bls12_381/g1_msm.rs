use super::{msm_required_gas, G1_INPUT_ITEM_LENGTH, SCALAR_LENGTH};
use crate::prelude::types::{make_address, Address, EthGas};
use crate::prelude::{Borrowed, Vec};
use crate::{EvmPrecompileResult, Precompile, PrecompileOutput};
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

/// BLS12-381 G1 MSM
pub struct BlsG1Msm;

impl BlsG1Msm {
    pub const ADDRESS: Address = make_address(0, 0xC);

    #[cfg(feature = "std")]
    fn execute(input: &[u8]) -> Result<Vec<u8>, ExitError> {
        use super::standalone::{extract_scalar_input, g1, NBITS};
        use blst::{blst_p1, blst_p1_affine, blst_p1_from_affine, blst_p1_to_affine, p1_affines};

        let k = input.len() / INPUT_LENGTH;
        let mut g1_points: Vec<blst_p1> = Vec::with_capacity(k);
        let mut scalars: Vec<u8> = Vec::with_capacity(k * SCALAR_LENGTH);
        for i in 0..k {
            let slice = &input[i * INPUT_LENGTH..i * INPUT_LENGTH + G1_INPUT_ITEM_LENGTH];

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
                    &input[i * INPUT_LENGTH + G1_INPUT_ITEM_LENGTH
                        ..i * INPUT_LENGTH + G1_INPUT_ITEM_LENGTH + SCALAR_LENGTH],
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

    #[cfg(not(feature = "std"))]
    fn execute(input: &[u8]) -> Result<Vec<u8>, ExitError> {
        use super::{extract_g1, padding_g1_result, FP_LENGTH};

        let k = input.len() / INPUT_LENGTH;
        let mut g1_input = crate::vec![0u8; k * (2 * FP_LENGTH + SCALAR_LENGTH)];
        for i in 0..k {
            let (p0_x, p0_y) =
                extract_g1(&input[i * INPUT_LENGTH..i * INPUT_LENGTH + G1_INPUT_ITEM_LENGTH])?;
            // Data offset for the points
            let offset = i * (2 * FP_LENGTH + SCALAR_LENGTH);
            // Check is p0 zero coordinate
            if input[i * INPUT_LENGTH..i * INPUT_LENGTH + G1_INPUT_ITEM_LENGTH]
                == [0; G1_INPUT_ITEM_LENGTH]
            {
                g1_input[offset] = 0x40;
            } else {
                g1_input[offset..offset + FP_LENGTH].copy_from_slice(p0_x);
                g1_input[offset + FP_LENGTH..offset + 2 * FP_LENGTH].copy_from_slice(p0_y);
            }
            // Set scalar
            let mut scalar =
                input[(i + 1) * INPUT_LENGTH - SCALAR_LENGTH..(i + 1) * INPUT_LENGTH].to_vec();
            scalar.reverse();
            g1_input[offset + 2 * FP_LENGTH..offset + 2 * FP_LENGTH + SCALAR_LENGTH]
                .copy_from_slice(&scalar);
        }

        let output = aurora_engine_sdk::bls12381_g1_multiexp(&g1_input[..]);
        Ok(padding_g1_result(&output))
    }
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

        let cost = Self::required_gas(input)?;
        if let Some(target_gas) = target_gas {
            if cost > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        let output = Self::execute(input)?;
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurora_engine_types::H160;

    #[test]
    fn bls12_381_g1_mul() {
        let precompile = BlsG1Msm;
        let ctx = Context {
            address: H160::zero(),
            caller: H160::zero(),
            apparent_value: 0.into(),
        };
        let input = hex::decode("\
               000000000000000000000000000000000b3a1dfe2d1b62538ed49648cb2a8a1d66bdc4f7a492eee59942ab810a306876a7d49e5ac4c6bb1613866c158ded993e\
			   000000000000000000000000000000001300956110f47ca8e2aacb30c948dfd046bf33f69bf54007d76373c5a66019454da45e3cf14ce2b9d53a50c9b4366aa3\
			   ac23d04ee3acc757aae6795532ce4c9f34534e506a4d843a26b052a040c79659")
            .expect("hex decoding failed");

        let res = precompile
            .run(&input, None, &ctx, false)
            .expect("precompile run should not fail");
        let expected = hex::decode("\
               000000000000000000000000000000001227b7021e9d3dc8bcbf5b346fc503f7f8576965769c5e22bb70056eef03c84b8c80290ae9ce20345770290c55549bce\
			   00000000000000000000000000000000188ddbbfb4ad2d34a8d3dc0ec92b70b63caa73ad7dea0cc9740bac2309b4bb11107912bd086379746e9a9bcd26d4db58")
            .expect("hex decoding failed");

        assert_eq!(res.output, expected);
    }

    #[test]
    fn bls12_381_g1_mul_extend() {
        let precompile = BlsG1Msm;
        let ctx = Context {
            address: H160::zero(),
            caller: H160::zero(),
            apparent_value: 0.into(),
        };
        let input = hex::decode("\
               0000000000000000000000000000000012196c5a43d69224d8713389285f26b98f86ee910ab3dd668e413738282003cc5b7357af9a7af54bb713d62255e80f56\
			   0000000000000000000000000000000006ba8102bfbeea4416b710c73e8cce3032c31c6269c44906f8ac4f7874ce99fb17559992486528963884ce429a992fee\
			   b3c940fe79b6966489b527955de7599194a9ac69a6ff58b8d99e7b1084f0464e\
			   00000000000000000000000000000000117dbe419018f67844f6a5e1b78a1e597283ad7b8ee7ac5e58846f5a5fd68d0da99ce235a91db3ec1cf340fe6b7afcdb\
			   0000000000000000000000000000000013316f23de032d25e912ae8dc9b54c8dba1be7cecdbb9d2228d7e8f652011d46be79089dd0a6080a73c82256ce5e4ed2\
			   4d0e25bf3f6fc9f4da25d21fdc71773f1947b7a8a775b8177f7eca990b05b71d\
			   0000000000000000000000000000000008ab7b556c672db7883ec47efa6d98bb08cec7902ebb421aac1c31506b177ac444ffa2d9b400a6f1cbdc6240c607ee11\
			   0000000000000000000000000000000016b7fa9adf4addc2192271ce7ad3c8d8f902d061c43b7d2e8e26922009b777855bffabe7ed1a09155819eabfa87f276f\
			   973f40c12c92b703d7b7848ef8b4466d40823aad3943a312b57432b91ff68be1\
			   0000000000000000000000000000000015ff9a232d9b5a8020a85d5fe08a1dcfb73ece434258fe0e2fddf10ddef0906c42dcb5f5d62fc97f934ba900f17beb33\
			   0000000000000000000000000000000009cfe4ee2241d9413c616462d7bac035a6766aeaab69c81e094d75b840df45d7e0dfac0265608b93efefb9a8728b98e4\
			   4c51f97bcdda93904ae26991b471e9ea942e2b5b8ed26055da11c58bc7b5002a\
			   0000000000000000000000000000000017a17b82e3bfadf3250210d8ef572c02c3610d65ab4d7366e0b748768a28ee6a1b51f77ed686a64f087f36f641e7dca9\
			   00000000000000000000000000000000077ea73d233ccea51dc4d5acecf6d9332bf17ae51598f4b394a5f62fb387e9c9aa1d6823b64a074f5873422ca57545d3\
			   8964d5867927bc3e35a0b4c457482373969bff5edff8a781d65573e07fd87b89\
			   000000000000000000000000000000000c1243478f4fbdc21ea9b241655947a28accd058d0cdb4f9f0576d32f09dddaf0850464550ff07cab5927b3e4c863ce9\
			   0000000000000000000000000000000015fb54db10ffac0b6cd374eb7168a8cb3df0a7d5f872d8e98c1f623deb66df5dd08ff4c3658f2905ec8bd02598bd4f90\
			   787c38b944eadbd03fd3187f450571740f6cd00e5b2e560165846eb800e5c944\
			   000000000000000000000000000000000328f09584b6d6c98a709fc22e184123994613aca95a28ac53df8523b92273eb6f4e2d9b2a7dcebb474604d54a210719\
			   000000000000000000000000000000001220ebde579911fe2e707446aaad8d3789fae96ae2e23670a4fd856ed82daaab704779eb4224027c1ed9460f39951a1b\
			   aaee7ae2a237e8e53560c79e7baa9adf9c00a0ea4d6f514e7a6832eb15cef1e1\
			   0000000000000000000000000000000002ebfa98aa92c32a29ebe17fcb1819ba82e686abd9371fcee8ea793b4c72b6464085044f818f1f5902396df0122830cb\
			   00000000000000000000000000000000001184715b8432ed190b459113977289a890f68f6085ea111466af15103c9c02467da33e01d6bff87fd57db6ccba442a\
			   dac6ed3ef45c1d7d3028f0f89e5458797996d3294b95bebe049b76c7d0db317c\
			   0000000000000000000000000000000009d6424e002439998e91cd509f85751ad25e574830c564e7568347d19e3f38add0cab067c0b4b0801785a78bcbeaf246\
			   000000000000000000000000000000000ef6d7db03ee654503b46ff0dbc3297536a422e963bda9871a8da8f4eeb98dedebd6071c4880b4636198f4c2375dc795\
			   bb30985756c3ca075114c92f231575d6befafe4084517f1166a47376867bd108\
			   0000000000000000000000000000000002d1cdb93191d1f9f0308c2c55d0208a071f5520faca7c52ab0311dbc9ba563bd33b5dd6baa77bf45ac2c3269e945f48\
			   00000000000000000000000000000000072a52106e6d7b92c594c4dacd20ef5fab7141e45c231457cd7e71463b2254ee6e72689e516fa6a8f29f2a173ce0a190\
			   fb730105809f64ea522983d6bbb62f7e2e8cbf702685e9be10e2ef71f8187672\
			   0000000000000000000000000000000000641642f6801d39a09a536f506056f72a619c50d043673d6d39aa4af11d8e3ded38b9c3bbc970dbc1bd55d68f94b50d\
			   0000000000000000000000000000000009ab050de356a24aea90007c6b319614ba2f2ed67223b972767117769e3c8e31ee4056494628fb2892d3d37afb6ac943\
			   b6a9408625b0ca8fcbfb21d34eec2d8e24e9a30d2d3b32d7a37d110b13afbfea\
			   000000000000000000000000000000000fd4893addbd58fb1bf30b8e62bef068da386edbab9541d198e8719b2de5beb9223d87387af82e8b55bd521ff3e47e2d\
			   000000000000000000000000000000000f3a923b76473d5b5a53501790cb02597bb778bdacb3805a9002b152d22241ad131d0f0d6a260739cbab2c2fe602870e\
			   3b77283d0a7bb9e17a27e66851792fdd605cc0a339028b8985390fd024374c76\
			   0000000000000000000000000000000002cb4b24c8aa799fd7cb1e4ab1aab1372113200343d8526ea7bc64dfaf926baf5d90756a40e35617854a2079cd07fba4\
			   0000000000000000000000000000000003327ca22bd64ebd673cc6d5b02b2a8804d5353c9d251637c4273ad08d581cc0d58da9bea27c37a0b3f4961dbafd276b\
			   dd994eae929aee7428fdda2e44f8cb12b10b91c83b22abc8bbb561310b62257c\
			   00000000000000000000000000000000024ad70f2b2105ca37112858e84c6f5e3ffd4a8b064522faae1ecba38fabd52a6274cb46b00075deb87472f11f2e67d9\
			   0000000000000000000000000000000010a502c8b2a68aa30d2cb719273550b9a3c283c35b2e18a01b0b765344ffaaa5cb30a1e3e6ecd3a53ab67658a5787681\
			   7010b134989c8368c7f831f9dd9f9a890e2c1435681107414f2e8637153bbf6a\
			   0000000000000000000000000000000000704cc57c8e0944326ddc7c747d9e7347a7f6918977132eea269f161461eb64066f773352f293a3ac458dc3ccd5026a\
			   000000000000000000000000000000001099d3c2bb2d082f2fdcbed013f7ac69e8624f4fcf6dfab3ee9dcf7fbbdb8c49ee79de40e887c0b6828d2496e3a6f768\
			   94c68bc8d91ac8c489ee87dbfc4b94c93c8bbd5fc04c27db8b02303f3a659054\
			   00000000000000000000000000000000130535a29392c77f045ac90e47f2e7b3cffff94494fe605aad345b41043f6663ada8e2e7ecd3d06f3b8854ef92212f42\
			   000000000000000000000000000000001699a3cc1f10cd2ed0dc68eb916b4402e4f12bf4746893bf70e26e209e605ea89e3d53e7ac52bd07713d3c8fc671931d\
			   b3682accc3939283b870357cf83683350baf73aa0d3d68bda82a0f6ae7e51746")
            .expect("hex decoding failed");

        let res = precompile
            .run(&input, None, &ctx, false)
            .expect("precompile run should not fail");
        let expected = hex::decode("\
               000000000000000000000000000000000b370fc4ca67fb0c3c270b1b4c4816ef953cd9f7cf6ad20e88099c40aace9c4bb3f4cd215e5796f65080c69c9f4d2a0f\
			   0000000000000000000000000000000007203220935ddc0190e2d7a99ec3f9231da550768373f9a5933dffd366f48146f8ea5fe5dee6539d925288083bb5a8f1")
            .expect("hex decoding failed");

        assert_eq!(res.output, expected);
    }
}

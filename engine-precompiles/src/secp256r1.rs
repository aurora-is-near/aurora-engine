//! # Precompile for secp256r1 operations.
//!
//! <https://eips.ethereum.org/EIPS/eip-7951>

use crate::prelude::types::{make_address, Address, EthGas};
use crate::{EvmPrecompileResult, Precompile, PrecompileOutput, Vec};
use aurora_evm::{Context, ExitError};
use p256::{
    ecdsa::{signature::hazmat::PrehashVerifier, Signature, VerifyingKey},
    EncodedPoint,
};

/// Base gas fee for secp256r1
pub const P256VERIFY_BASE_GAS_FEE: u64 = 6900;

/// Input length for secp256r1: 32 x 5 = 160 bytes
const INPUT_LENGTH: usize = 160;

/// Success result: 32 bytes with last byte set to 1
const SUCCESS_RESULT: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
];

pub struct Secp256r1;

impl Secp256r1 {
    pub const ADDRESS: Address = make_address(0, 0x100);

    /// Executes the P256VERIFY operation (ECDSA signature verification over secp256r1).
    ///
    /// This function implements the specification defined in
    /// [EIP-7951](https://eips.ethereum.org/EIPS/eip-7951)
    ///
    /// # Specification Compliance
    ///
    /// 1. **Input Validation**: Checks strict 160-byte input length.
    /// 2. **Signature Validation**: Ensures `r` and `s` are in the range `(0, n)`.
    /// 3. **Public Key Validation**: Ensures coordinates are in `[0, p)`, satisfy the curve equation,
    ///    and the point is not at infinity.
    /// 4. **Verification**: Performs ECDSA verification including the critical modular reduction
    ///    fix ($r' \equiv r \pmod n$) required to prevent RIP-7212 vulnerabilities.
    ///
    /// # Returns
    ///
    /// * `Vec<u8>` - 32-byte success result (`0x00...01`) if verification passes.
    /// * `None` - If any validation fails or signature is invalid. This results in empty output
    ///   but consumes the full gas cost, as per "Gas Burning on Error" section.
    fn execute(input: &[u8]) -> Option<Vec<u8>> {
        // 1. Input length check
        if input.len() != INPUT_LENGTH {
            return None;
        }

        // 2. Parse Inputs
        // Message hash (h)
        let h_bytes = &input[0..32];
        // Signature component (r)
        let r_bytes = &input[32..64];
        // Signature component (s)
        let s_bytes = &input[64..96];
        // Public key x-coordinate (qx)
        let qx_bytes = &input[96..128];
        // Public key y-coordinate (qy)
        let qy_bytes = &input[128..160];

        // 3. Signature Component Validation
        // Spec: "Both r and s MUST satisfy 0 < r < n and 0 < s < n"
        // `Signature::from_scalars` returns an Error if scalars are zero or >= group order (n).
        let signature = Signature::from_scalars(
            *p256::FieldBytes::from_slice(r_bytes),
            *p256::FieldBytes::from_slice(s_bytes),
        )
        .ok()?;

        // 4. Public Key Validation
        // Spec: "Both qx and qy MUST satisfy 0 <= qx < p and 0 <= qy < p"
        // Spec: "The point (qx, qy) MUST satisfy the curve equation"
        // Spec: "The point (qx, qy) MUST NOT be the point at infinity"
        //
        // We reconstruct the point from raw coordinates (0x04 || x || y implicit format).
        // `VerifyingKey::from_encoded_point` validates that the point satisfies y^2 = x^3 + ax + b.
        // Since secp256r1 coefficient b != 0, the point at infinity (0, 0) does not satisfy
        // the curve equation and will be rejected here.
        let mut pubkey_bytes = [0u8; 64];
        pubkey_bytes[0..32].copy_from_slice(qx_bytes);
        pubkey_bytes[32..64].copy_from_slice(qy_bytes);

        let encoded_point = EncodedPoint::from_untagged_bytes(&pubkey_bytes.into());
        let public_key = VerifyingKey::from_encoded_point(&encoded_point).ok()?;

        // 5. Signature Verification
        // Spec: "s1 = s^(-1) (mod n)"
        // Spec: "R' = (h * s1) * G + (r * s1) * (qx, qy)"
        // Spec: "If R' is the point at infinity: return"
        // Spec: "if r' == r (mod n): return success"
        //
        // The `verify_prehash` method implements FIPS 186-5 ECDSA verification.
        // Crucially, it handles the modular reduction of the computed x-coordinate ($r' \pmod n$)
        // before comparing it with the signature component $r$. This addresses the
        // consensus vulnerability found in original RIP-7212.
        if public_key.verify_prehash(h_bytes, &signature).is_ok() {
            // Spec: "Output is 32 bytes... 0x00...01 for valid signatures"
            Some(Vec::from(&SUCCESS_RESULT[..]))
        } else {
            // Spec: "return `` (failure)"
            None
        }
    }
}

impl Precompile for Secp256r1 {
    fn required_gas(_input: &[u8]) -> Result<EthGas, ExitError>
    where
        Self: Sized,
    {
        Ok(EthGas::new(P256VERIFY_BASE_GAS_FEE))
    }

    fn run(
        &self,
        input: &[u8],
        target_gas: Option<EthGas>,
        _context: &Context,
        _is_static: bool,
    ) -> EvmPrecompileResult {
        let cost = Self::required_gas(input)?;
        if let Some(target_gas) = target_gas {
            if cost > target_gas {
                return Err(ExitError::OutOfGas);
            }
        }

        // Return empty output on failure according to EIP-7951
        let output = Self::execute(input).unwrap_or_default();
        Ok(PrecompileOutput::without_logs(cost, output))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn context() -> Context {
        Context {
            address: Secp256r1::ADDRESS.raw(),
            caller: Secp256r1::ADDRESS.raw(),
            apparent_value: 0u128.into(),
        }
    }

    /// Test vectors from <https://github.com/daimo-eth/p256-verifier/tree/master/test-vectors>
    #[test]
    fn test_sig_verify() {
        let inputs = vec![
            ("4cee90eb86eaa050036147a12d49004b6b9c72bd725d39d4785011fe190f0b4da73bd4903f0ce3b639bbbf6e8e80d16931ff4bcf5993d58468e8fb19086e8cac36dbcd03009df8c59286b162af3bd7fcc0450c9aa81be5d10d312af6c66b1d604aebd3099c618202fcfe16ae7770b0c49ab5eadf74b754204a3bb6060e44eff37618b065f9832de4ca6ca971a7a1adc826d0f7c00181a5fb2ddf79ae00b4e10e", true),
            ("3fec5769b5cf4e310a7d150508e82fb8e3eda1c2c94c61492d3bd8aea99e06c9e22466e928fdccef0de49e3503d2657d00494a00e764fd437bdafa05f5922b1fbbb77c6817ccf50748419477e843d5bac67e6a70e97dde5a57e0c983b777e1ad31a80482dadf89de6302b1988c82c29544c9c07bb910596158f6062517eb089a2f54c9a0f348752950094d3228d3b940258c75fe2a413cb70baa21dc2e352fc5", true),
            ("e775723953ead4a90411a02908fd1a629db584bc600664c609061f221ef6bf7c440066c8626b49daaa7bf2bcc0b74be4f7a1e3dcf0e869f1542fe821498cbf2de73ad398194129f635de4424a07ca715838aefe8fe69d1a391cfa70470795a80dd056866e6e1125aff94413921880c437c9e2570a28ced7267c8beef7e9b2d8d1547d76dfcf4bee592f5fefe10ddfb6aeb0991c5b9dbbee6ec80d11b17c0eb1a", true),
            ("b5a77e7a90aa14e0bf5f337f06f597148676424fae26e175c6e5621c34351955289f319789da424845c9eac935245fcddd805950e2f02506d09be7e411199556d262144475b1fa46ad85250728c600c53dfd10f8b3f4adf140e27241aec3c2da3a81046703fccf468b48b145f939efdbb96c3786db712b3113bb2488ef286cdcef8afe82d200a5bb36b5462166e8ce77f2d831a52ef2135b2af188110beaefb1", true),
            ("858b991cfd78f16537fe6d1f4afd10273384db08bdfc843562a22b0626766686f6aec8247599f40bfe01bec0e0ecf17b4319559022d4d9bf007fe929943004eb4866760dedf31b7c691f5ce665f8aae0bda895c23595c834fecc2390a5bcc203b04afcacbb4280713287a2d0c37e23f7513fab898f2c1fefa00ec09a924c335d9b629f1d4fb71901c3e59611afbfea354d101324e894c788d1c01f00b3c251b2", true),
            ("3cee90eb86eaa050036147a12d49004b6b9c72bd725d39d4785011fe190f0b4da73bd4903f0ce3b639bbbf6e8e80d16931ff4bcf5993d58468e8fb19086e8cac36dbcd03009df8c59286b162af3bd7fcc0450c9aa81be5d10d312af6c66b1d604aebd3099c618202fcfe16ae7770b0c49ab5eadf74b754204a3bb6060e44eff37618b065f9832de4ca6ca971a7a1adc826d0f7c00181a5fb2ddf79ae00b4e10e", false),
            ("afec5769b5cf4e310a7d150508e82fb8e3eda1c2c94c61492d3bd8aea99e06c9e22466e928fdccef0de49e3503d2657d00494a00e764fd437bdafa05f5922b1fbbb77c6817ccf50748419477e843d5bac67e6a70e97dde5a57e0c983b777e1ad31a80482dadf89de6302b1988c82c29544c9c07bb910596158f6062517eb089a2f54c9a0f348752950094d3228d3b940258c75fe2a413cb70baa21dc2e352fc5", false),
            ("f775723953ead4a90411a02908fd1a629db584bc600664c609061f221ef6bf7c440066c8626b49daaa7bf2bcc0b74be4f7a1e3dcf0e869f1542fe821498cbf2de73ad398194129f635de4424a07ca715838aefe8fe69d1a391cfa70470795a80dd056866e6e1125aff94413921880c437c9e2570a28ced7267c8beef7e9b2d8d1547d76dfcf4bee592f5fefe10ddfb6aeb0991c5b9dbbee6ec80d11b17c0eb1a", false),
            ("c5a77e7a90aa14e0bf5f337f06f597148676424fae26e175c6e5621c34351955289f319789da424845c9eac935245fcddd805950e2f02506d09be7e411199556d262144475b1fa46ad85250728c600c53dfd10f8b3f4adf140e27241aec3c2da3a81046703fccf468b48b145f939efdbb96c3786db712b3113bb2488ef286cdcef8afe82d200a5bb36b5462166e8ce77f2d831a52ef2135b2af188110beaefb1", false),
            ("958b991cfd78f16537fe6d1f4afd10273384db08bdfc843562a22b0626766686f6aec8247599f40bfe01bec0e0ecf17b4319559022d4d9bf007fe929943004eb4866760dedf31b7c691f5ce665f8aae0bda895c23595c834fecc2390a5bcc203b04afcacbb4280713287a2d0c37e23f7513fab898f2c1fefa00ec09a924c335d9b629f1d4fb71901c3e59611afbfea354d101324e894c788d1c01f00b3c251b2", false),
            ("4cee90eb86eaa050036147a12d49004b6a", false),
            ("4cee90eb86eaa050036147a12d49004b6a958b991cfd78f16537fe6d1f4afd10273384db08bdfc843562a22b0626766686f6aec8247599f40bfe01bec0e0ecf17b4319559022d4d9bf007fe929943004eb4866760dedf319", false),
            ("4cee90eb86eaa050036147a12d49004b6b9c72bd725d39d4785011fe190f0b4da73bd4903f0ce3b639bbbf6e8e80d16931ff4bcf5993d58468e8fb19086e8cac36dbcd03009df8c59286b162af3bd7fcc0450c9aa81be5d10d312af6c66b1d604aebd3099c618202fcfe16ae7770b0c49ab5eadf74b754204a3bb6060e44eff37618b065f9832de4ca6ca971a7a1adc826d0f7c00181a5fb2ddf79ae00b4e10e00", false),
            ("4cee90eb86eaa050036147a12d49004b6b9c72bd725d39d4785011fe190f0b4dffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff4aebd3099c618202fcfe16ae7770b0c49ab5eadf74b754204a3bb6060e44eff37618b065f9832de4ca6ca971a7a1adc826d0f7c00181a5fb2ddf79ae00b4e10e", false),
            ("4cee90eb86eaa050036147a12d49004b6b9c72bd725d39d4785011fe190f0b4da73bd4903f0ce3b639bbbf6e8e80d16931ff4bcf5993d58468e8fb19086e8cac36dbcd03009df8c59286b162af3bd7fcc0450c9aa81be5d10d312af6c66b1d6000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000", false),
            ("b5a77e7a90aa14e0bf5f337f06f597148676424fae26e175c6e5621c34351955289f319789da424845c9eac935245fcddd805950e2f02506d09be7e411199556d262144475b1fa46ad85250728c600c53dfd10f8b3f4adf140e27241aec3c2daaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaef8afe82d200a5bb36b5462166e8ce77f2d831a52ef2135b2af188110beaefb1", false),
        ];
        let p = Secp256r1;
        for (input_hex, expect_success) in inputs {
            let input = hex::decode(input_hex).unwrap();
            let res = p.run(&input, None, &context(), false).unwrap();
            if expect_success {
                assert_eq!(
                    res.output,
                    SUCCESS_RESULT.to_vec(),
                    "Input hex: {input_hex}",
                );
            } else {
                assert_eq!(res.output.len(), 0, "Input hex: {input_hex}");
            }
        }
    }

    #[test]
    fn test_not_enough_gas_errors() {
        let input_hex = "4cee90eb86eaa050036147a12d49004b6b9c72bd725d39d4785011fe190f0b4da73bd4903f0ce3b639bbbf6e8e80d16931ff4bcf5993d58468e8fb19086e8cac36dbcd03009df8c59286b162af3bd7fcc0450c9aa81be5d10d312af6c66b1d604aebd3099c618202fcfe16ae7770b0c49ab5eadf74b754204a3bb6060e44eff37618b065f9832de4ca6ca971a7a1adc826d0f7c00181a5fb2ddf79ae00b4e10e";
        let p = Secp256r1;

        let input = hex::decode(input_hex).unwrap();
        let err = p
            .run(&input, Some(EthGas::new(2_500)), &context(), false)
            .unwrap_err();
        assert_eq!(err, ExitError::OutOfGas);
    }

    #[test]
    fn test_eip7951_spec_compliance_edge_cases() {
        let p = Secp256r1;

        let valid_full_hex = "4cee90eb86eaa050036147a12d49004b6b9c72bd725d39d4785011fe190f0b4da73bd4903f0ce3b639bbbf6e8e80d16931ff4bcf5993d58468e8fb19086e8cac36dbcd03009df8c59286b162af3bd7fcc0450c9aa81be5d10d312af6c66b1d604aebd3099c618202fcfe16ae7770b0c49ab5eadf74b754204a3bb6060e44eff37618b065f9832de4ca6ca971a7a1adc826d0f7c00181a5fb2ddf79ae00b4e10e";

        // Slicing string indices (each byte is 2 hex chars)
        let h = &valid_full_hex[0..64];
        let r = &valid_full_hex[64..128];
        let s = &valid_full_hex[128..192];
        let qx = &valid_full_hex[192..256];
        let qy = &valid_full_hex[256..320];

        // Constants for boundaries (hex strings must be exactly 64 chars)
        let val_p = "ffffffff00000001000000000000000000000000ffffffffffffffffffffffff";
        let val_n = "ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551";
        let val_zero = "0000000000000000000000000000000000000000000000000000000000000000";

        let failures = [
            // Case 1: Public Key is Point at Infinity (0, 0)
            // Spec: "The point (qx, qy) MUST NOT be the point at infinity (represented as (0, 0))"
            format!("{h}{r}{s}{val_zero}{val_zero}"),
            // Case 2: Public Key X coordinate >= P
            // Spec: "Both qx and qy MUST satisfy 0 <= qx < p"
            format!("{h}{r}{s}{val_p}{qy}"),
            // Case 3: Public Key Y coordinate >= P
            // Spec: "Both qx and qy MUST satisfy ... 0 <= qy < p"
            format!("{h}{r}{s}{qx}{val_p}"),
            // Case 4: Scalar r is Zero
            // Spec: "Both r and s MUST satisfy 0 < r < n"
            format!("{h}{val_zero}{s}{qx}{qy}"),
            // Case 5: Scalar s is Zero
            // Spec: "Both r and s MUST satisfy ... 0 < s < n"
            format!("{h}{r}{val_zero}{qx}{qy}"),
            // Case 6: Scalar r >= n
            // Spec: "Both r and s MUST satisfy 0 < r < n"
            format!("{h}{val_n}{s}{qx}{qy}"),
            // Case 7: Scalar s >= n
            // Spec: "Both r and s MUST satisfy ... 0 < s < n"
            format!("{h}{r}{val_n}{qx}{qy}"),
        ];

        for (i, input_hex) in failures.iter().enumerate() {
            let input = hex::decode(input_hex).unwrap_or_else(|e| {
                panic!("Failed to decode hex for Case {i}: {e}");
            });

            // Ensure we built the test vector correctly
            assert_eq!(input.len(), 160, "Case {i} input length is not 160 bytes");

            let res = p.run(&input, None, &context(), false).unwrap();

            // Should return empty bytes (failure), NOT revert
            assert_eq!(
                res.output.len(),
                0,
                "Case {i} expected failure (empty output), got success",
            );
        }
    }

    #[test]
    fn test_eip7951_input_length_validation() {
        let p = Secp256r1;

        // A valid 160-byte input (derived from valid vector)
        let valid_hex = "4cee90eb86eaa050036147a12d49004b6b9c72bd725d39d4785011fe190f0b4da73bd4903f0ce3b639bbbf6e8e80d16931ff4bcf5993d58468e8fb19086e8cac36dbcd03009df8c59286b162af3bd7fcc0450c9aa81be5d10d312af6c66b1d604aebd3099c618202fcfe16ae7770b0c49ab5eadf74b754204a3bb6060e44eff37618b065f9832de4ca6ca971a7a1adc826d0f7c00181a5fb2ddf79ae00b4e10e";
        let valid_bytes = hex::decode(valid_hex).unwrap();
        assert_eq!(valid_bytes.len(), 160);

        // Test Case 1: Empty input
        // Spec: "if input_length != 160: return"
        let res_empty = p.run(&[], None, &context(), false).unwrap();
        assert_eq!(
            res_empty.output.len(),
            0,
            "Empty input should return empty bytes"
        );

        // Test Case 2: Too short (159 bytes)
        let input_short = &valid_bytes[0..159];
        let res_short = p.run(input_short, None, &context(), false).unwrap();
        assert_eq!(
            res_short.output.len(),
            0,
            "159 bytes input should return empty bytes"
        );

        // Test Case 3: Too long (161 bytes)
        let mut input_long = valid_bytes.clone();
        input_long.push(0x00); // Add 1 byte
        let res_long = p.run(&input_long, None, &context(), false).unwrap();
        assert_eq!(
            res_long.output.len(),
            0,
            "161 bytes input should return empty bytes"
        );

        // Test Case 4: Random length (e.g. 32 bytes)
        let input_32 = &valid_bytes[0..32];
        let res_32 = p.run(input_32, None, &context(), false).unwrap();
        assert_eq!(
            res_32.output.len(),
            0,
            "32 bytes input should return empty bytes"
        );
    }
}

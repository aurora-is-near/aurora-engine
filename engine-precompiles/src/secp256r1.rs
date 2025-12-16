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

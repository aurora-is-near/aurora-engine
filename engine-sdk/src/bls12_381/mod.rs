#[cfg(feature = "contract")]
mod contract;
#[cfg(not(feature = "contract"))]
mod standalone;

#[cfg(feature = "contract")]
pub use contract::{g1_add, g1_msm, g2_add, g2_msm, map_fp_to_g1, map_fp2_to_g2, pairing_check};
#[cfg(not(feature = "contract"))]
pub use standalone::{g1_add, g1_msm, g2_add, g2_msm, map_fp_to_g1, map_fp2_to_g2, pairing_check};

/// Finite field element padded input length.
pub const PADDED_FP_LENGTH: usize = 64;
/// Quadratic extension of finite field element input length.
pub const PADDED_FP2_LENGTH: usize = 128;
/// Input length of `g1_mul` operation.
pub const G1_MUL_INPUT_LENGTH: usize = 160;
/// Input length of `g2_mul` operation.
pub const G2_MUL_INPUT_LENGTH: usize = 288;
/// Input length of pairing operation.
pub const PAIRING_INPUT_LENGTH: usize = 384;

/// Length of each element in a g1 operation input.
const G1_INPUT_ITEM_LENGTH: usize = 128;
/// Length of each element in a g2 operation input.
const G2_INPUT_ITEM_LENGTH: usize = 256;
/// Finite field element input length.
const FP_LENGTH: usize = 48;
/// Input elements padding length.
const PADDING_LENGTH: usize = 16;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum Bls12381Error {
    Padding,
    UsizeConversion,
    G1InputLength,
    G2InputLength,
    ElementNotInG1,
    ElementNotInG2,
    InvalidFpValue,
    ScalarLength,
}

impl AsRef<&'static str> for Bls12381Error {
    fn as_ref(&self) -> &&'static str {
        match self {
            Self::Padding => &"ERR_BLS12_PADDING",
            Self::UsizeConversion => &"ERR_BLS12_USIZE_CONVERSION",
            Self::G1InputLength => &"ERR_BLS12_G1_INPUT_LENGTH",
            Self::G2InputLength => &"ERR_BLS12_G2_INPUT_LENGTH",
            Self::ElementNotInG1 => &"ERR_BLS12_ELEMENT_NOT_IN_G1",
            Self::ElementNotInG2 => &"ERR_BLS12_ELEMENT_NOT_IN_G2",
            Self::InvalidFpValue => &"ERR_BLS12_FP_VALUE",
            Self::ScalarLength => &"ERR_BLS12_SCALAR_LENGTH",
        }
    }
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

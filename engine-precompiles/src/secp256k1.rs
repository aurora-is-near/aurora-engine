use crate::PrecompileError;
#[cfg(not(feature = "contract"))]
use aurora_engine_sdk::ECRecoverErr;
use aurora_engine_types::types::{make_address, Address};
#[cfg(not(feature = "contract"))]
use aurora_engine_types::ToString;
use aurora_engine_types::{Borrowed, Vec, H256};

pub const ADDRESS: Address = make_address(0, 1);

pub(crate) const ECRECOVER_BASE: u64 = 3_000;
pub(crate) const INPUT_LEN: usize = 128;
pub(crate) const SIGNATURE_LENGTH: usize = 65;

/// See: `https://ethereum.github.io/yellowpaper/paper.pdf`
/// See: `https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions`
/// See: `https://etherscan.io/address/0000000000000000000000000000000000000001`
// Quite a few library methods rely on this and that should be changed. This
// should only be for precompiles.
pub fn ecrecover(
    hash: H256,
    signature: &[u8; SIGNATURE_LENGTH],
) -> Result<Address, PrecompileError> {
    #[cfg(feature = "contract")]
    return aurora_engine_sdk::ecrecover(hash, signature)
        .map_err(|e| PrecompileError::Other(Borrowed(e.as_str())));

    #[cfg(not(feature = "contract"))]
    internal_impl(hash, signature)
}

#[cfg(not(feature = "contract"))]
fn internal_impl(hash: H256, signature: &[u8]) -> Result<Address, PrecompileError> {
    use aurora_engine_types::Cow::Owned;
    use sha3::Digest;

    let hash = libsecp256k1::Message::parse_slice(hash.as_bytes())
        .map_err(|e| PrecompileError::Other(Owned(e.to_string())))?;
    let v = signature[64];
    let signature = libsecp256k1::Signature::parse_standard_slice(&signature[0..64])
        .map_err(|_| PrecompileError::Other(Borrowed(ECRecoverErr.as_str())))?;
    let bit = match v {
        0..=26 => v,
        _ => v - 27,
    };

    if let Ok(recovery_id) = libsecp256k1::RecoveryId::parse(bit) {
        if let Ok(public_key) = libsecp256k1::recover(&hash, &signature, &recovery_id) {
            // recover returns a 65-byte key, but addresses come from the raw 64-byte key
            let r = sha3::Keccak256::digest(&public_key.serialize()[1..]);
            return Address::try_from_slice(&r[12..])
                .map_err(|_| PrecompileError::Other(Borrowed("ERR_INCORRECT_ADDRESS")));
        }
    }

    Err(PrecompileError::Other(Borrowed(ECRecoverErr.as_str())))
}

pub const fn required_gas() -> Result<u64, PrecompileError> {
    Ok(ECRECOVER_BASE)
}

pub fn run(input: &[u8], gas_limit: u64) -> crate::PrecompileResult {
    let gas_used = required_gas()?;
    if gas_used > gas_limit {
        return Err(PrecompileError::OutOfGas);
    }

    let mut input = input.to_vec();
    input.resize(INPUT_LEN, 0);

    let mut hash = [0; 32];
    hash.copy_from_slice(&input[0..32]);

    let mut v = [0; 32];
    v.copy_from_slice(&input[32..64]);

    let mut signature = [0; SIGNATURE_LENGTH]; // signature is (r, s, v), typed (uint256, uint256, uint8)
    signature[0..32].copy_from_slice(&input[64..96]); // r
    signature[32..64].copy_from_slice(&input[96..128]); // s

    let v_bit = match v[31] {
        27 | 28 if v[..31] == [0; 31] => v[31] - 27,
        _ => {
            return Ok((gas_used, Vec::new()));
        }
    };
    signature[64] = v_bit; // v

    let address_res = ecrecover(H256::from_slice(&hash), &signature);
    let output = address_res
        .map(|a| {
            let mut output = [0u8; 32];
            output[12..32].copy_from_slice(a.as_bytes());
            output.to_vec()
        })
        .unwrap_or_default();

    Ok((gas_used, output))
}

/// original source:
/// https://github.com/near/near-sdk-rs/blob/master/near-sdk/src/types/public_key.rs
///
/// This is a stripped down implementation that should be deprecated
/// if / when the engine migrates to near-sdk
///
///
/// Everything below is copy-pasted from the original source
/// except the following changes:
/// - removed "as" conversion to fix clippy warnings
/// - implemented conversion to NearPublicKey
///
use crate::parameters::NearPublicKey;
use crate::{fmt, str, str::FromStr, String, Vec};
use bs58::decode::Error as B58Error;

/// PublicKey curve
#[derive(Debug, Clone, Copy, PartialOrd, Ord, Eq, PartialEq)]
#[repr(u8)]
pub enum CurveType {
    ED25519 = 0,
    SECP256K1 = 1,
}

impl CurveType {
    fn from_u8(val: u8) -> Result<Self, ParsePublicKeyError> {
        match val {
            0 => Ok(CurveType::ED25519),
            1 => Ok(CurveType::SECP256K1),
            _ => Err(ParsePublicKeyError {
                kind: ParsePublicKeyErrorKind::UnknownCurve,
            }),
        }
    }

    /// Get the length of bytes associated to this CurveType
    const fn data_len(&self) -> usize {
        match self {
            CurveType::ED25519 => 32,
            CurveType::SECP256K1 => 64,
        }
    }
}

impl FromStr for CurveType {
    type Err = ParsePublicKeyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.eq_ignore_ascii_case("ed25519") {
            Ok(CurveType::ED25519)
        } else if value.eq_ignore_ascii_case("secp256k1") {
            Ok(CurveType::SECP256K1)
        } else {
            Err(ParsePublicKeyError {
                kind: ParsePublicKeyErrorKind::UnknownCurve,
            })
        }
    }
}
//
/// Public key in a binary format with base58 string serialization with human-readable curve.
/// The key types currently supported are `secp256k1` and `ed25519`.
///
/// Ed25519 public keys accepted are 32 bytes and secp256k1 keys are the uncompressed 64 format.
///
/// # Example
/// ```
/// use aurora_engine_types::public_key::PublicKey;
///
/// // Compressed ed25519 key
/// let ed: PublicKey = "ed25519:6E8sCci9badyRkXb3JoRpBj5p8C6Tw41ELDZoiihKEtp".parse()
///             .unwrap();
///
/// // Uncompressed secp256k1 key
/// let secp256k1: PublicKey = "secp256k1:qMoRgcoXai4mBPsdbHi1wfyxF9TdbPCF4qSDQTRP3TfescSRoUdSx6nmeQoN3aiwGzwMyGXAb1gUjBTv5AY8DXj"
///             .parse()
///             .unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq, Hash)]
pub struct PublicKey {
    data: Vec<u8>,
}

impl PublicKey {
    fn split_key_type_data(value: &str) -> Result<(CurveType, &str), ParsePublicKeyError> {
        if let Some(idx) = value.find(':') {
            let (prefix, key_data) = value.split_at(idx);
            Ok((prefix.parse::<CurveType>()?, &key_data[1..]))
        } else {
            // If there is no Default is ED25519.
            Ok((CurveType::ED25519, value))
        }
    }

    fn from_parts(curve: CurveType, data: Vec<u8>) -> Result<Self, ParsePublicKeyError> {
        let expected_length = curve.data_len();
        if data.len() != expected_length {
            return Err(ParsePublicKeyError {
                kind: ParsePublicKeyErrorKind::InvalidLength(data.len()),
            });
        }
        let mut bytes = Vec::with_capacity(1 + expected_length);
        bytes.push(curve.into());
        bytes.extend(data);

        Ok(Self { data: bytes })
    }

    /// Returns a byte slice of this `PublicKey`'s contents.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Converts a `PublicKey` into a byte vector.
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    /// Get info about the CurveType for this public key
    pub fn curve_type(&self) -> CurveType {
        CurveType::from_u8(self.data[0]).unwrap()
    }
}

impl From<PublicKey> for Vec<u8> {
    fn from(v: PublicKey) -> Vec<u8> {
        v.data
    }
}

impl TryFrom<Vec<u8>> for PublicKey {
    type Error = ParsePublicKeyError;

    fn try_from(data: Vec<u8>) -> Result<Self, Self::Error> {
        if data.is_empty() {
            return Err(ParsePublicKeyError {
                kind: ParsePublicKeyErrorKind::InvalidLength(data.len()),
            });
        }

        let curve = CurveType::from_u8(data[0])?;
        if data.len() != curve.data_len() + 1 {
            return Err(ParsePublicKeyError {
                kind: ParsePublicKeyErrorKind::InvalidLength(data.len()),
            });
        }
        Ok(Self { data })
    }
}

impl From<&PublicKey> for String {
    fn from(str_public_key: &PublicKey) -> Self {
        match str_public_key.curve_type() {
            CurveType::ED25519 => [
                "ed25519:",
                &bs58::encode(&str_public_key.data[1..]).into_string(),
            ]
            .concat(),
            CurveType::SECP256K1 => [
                "secp256k1:",
                &bs58::encode(&str_public_key.data[1..]).into_string(),
            ]
            .concat(),
        }
    }
}

impl FromStr for PublicKey {
    type Err = ParsePublicKeyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (curve, key_data) = PublicKey::split_key_type_data(value)?;
        let data = bs58::decode(key_data).into_vec()?;
        Self::from_parts(curve, data)
    }
}
#[derive(Debug)]
pub struct ParsePublicKeyError {
    kind: ParsePublicKeyErrorKind,
}

#[derive(Debug)]
enum ParsePublicKeyErrorKind {
    InvalidLength(usize),
    Base58(B58Error),
    UnknownCurve,
}

impl fmt::Display for ParsePublicKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ParsePublicKeyErrorKind::InvalidLength(l) => {
                write!(f, "invalid length of the public key, expected 32 got {}", l)
            }
            ParsePublicKeyErrorKind::Base58(e) => write!(f, "base58 decoding error: {}", e),
            ParsePublicKeyErrorKind::UnknownCurve => write!(f, "unknown curve kind"),
        }
    }
}

impl From<B58Error> for ParsePublicKeyError {
    fn from(e: B58Error) -> Self {
        Self {
            kind: ParsePublicKeyErrorKind::Base58(e),
        }
    }
}

//
// custom (non-near-sdk) implementations
//

impl From<PublicKey> for NearPublicKey {
    fn from(val: PublicKey) -> Self {
        return match val.curve_type() {
            CurveType::ED25519 => NearPublicKey::Ed25519(val.as_bytes()[1..].try_into().unwrap()),
            CurveType::SECP256K1 => {
                NearPublicKey::Secp256k1(val.as_bytes()[1..].try_into().unwrap())
            }
        };
    }
}

impl From<CurveType> for u8 {
    fn from(val: CurveType) -> u8 {
        match val {
            CurveType::ED25519 => 0,
            CurveType::SECP256K1 => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryInto;
    use std::str::FromStr;

    fn expected_key() -> PublicKey {
        let mut key = vec![CurveType::ED25519.into()];
        key.extend(
            bs58::decode("6E8sCci9badyRkXb3JoRpBj5p8C6Tw41ELDZoiihKEtp")
                .into_vec()
                .unwrap(),
        );
        key.try_into().unwrap()
    }

    #[test]
    fn test_public_key_from_str() {
        let key =
            PublicKey::from_str("ed25519:6E8sCci9badyRkXb3JoRpBj5p8C6Tw41ELDZoiihKEtp").unwrap();
        assert_eq!(key, expected_key());
    }

    #[test]
    fn test_public_key_to_string() {
        let key: PublicKey = expected_key();
        let actual: String = String::try_from(&key).unwrap();
        assert_eq!(
            actual,
            "ed25519:6E8sCci9badyRkXb3JoRpBj5p8C6Tw41ELDZoiihKEtp"
        );
    }
}

use crate::{fmt, str::FromStr, String, ToString};
#[cfg(not(feature = "borsh-compat"))]
use borsh::{maybestd::io, BorshDeserialize, BorshSerialize};
#[cfg(feature = "borsh-compat")]
use borsh_compat::{maybestd::io, BorshDeserialize, BorshSerialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicKey {
    /// ed25519 public keys are 32 bytes
    Ed25519([u8; 32]),
    /// secp256k1 keys are in the uncompressed 64 byte format
    Secp256k1([u8; 64]),
}

impl PublicKey {
    #[must_use]
    pub fn key_data(&self) -> &[u8] {
        match self {
            Self::Ed25519(data) => &data[..],
            Self::Secp256k1(data) => &data[..],
        }
    }
}

impl BorshSerialize for PublicKey {
    fn serialize<W: io::Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        match self {
            Self::Ed25519(public_key) => {
                BorshSerialize::serialize(&0u8, writer)?;
                writer.write_all(public_key)?;
            }
            Self::Secp256k1(public_key) => {
                BorshSerialize::serialize(&1u8, writer)?;
                writer.write_all(public_key)?;
            }
        }
        Ok(())
    }
}

#[cfg(not(feature = "borsh-compat"))]
impl BorshDeserialize for PublicKey {
    fn deserialize_reader<R: io::Read>(rd: &mut R) -> io::Result<Self> {
        let key_type = KeyType::try_from(u8::deserialize_reader(rd)?)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        match key_type {
            KeyType::Ed25519 => Ok(Self::Ed25519(BorshDeserialize::deserialize_reader(rd)?)),
            KeyType::Secp256k1 => Ok(Self::Secp256k1(BorshDeserialize::deserialize_reader(rd)?)),
        }
    }
}

#[cfg(feature = "borsh-compat")]
impl BorshDeserialize for PublicKey {
    fn deserialize(buf: &mut &[u8]) -> io::Result<Self> {
        let key_type = <u8 as BorshDeserialize>::deserialize(buf).and_then(KeyType::try_from)?;

        match key_type {
            KeyType::Ed25519 => Ok(Self::Ed25519(BorshDeserialize::deserialize(buf)?)),
            KeyType::Secp256k1 => Ok(Self::Secp256k1(BorshDeserialize::deserialize(buf)?)),
        }
    }
}

impl serde::Serialize for PublicKey {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<<S as serde::Serializer>::Ok, <S as serde::Serializer>::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as serde::Deserializer<'de>>::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;
        s.parse()
            .map_err(|_| serde::de::Error::custom("PublicKey decode error"))
    }
}

impl FromStr for PublicKey {
    type Err = DecodeBs58Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (key_type, key_data) = split_key_type_data(value)?;
        Ok(match key_type {
            KeyType::Ed25519 => Self::Ed25519(decode_bs58(key_data)?),
            KeyType::Secp256k1 => Self::Secp256k1(decode_bs58(key_data)?),
        })
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let (key_type, key_data) = match self {
            Self::Ed25519(public_key) => (KeyType::Ed25519, &public_key[..]),
            Self::Secp256k1(public_key) => (KeyType::Secp256k1, &public_key[..]),
        };
        write!(fmt, "{}:{}", key_type, Bs58(key_data))
    }
}

pub enum KeyType {
    Ed25519,
    Secp256k1,
}

impl TryFrom<u8> for KeyType {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Ed25519),
            1 => Ok(Self::Secp256k1),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Wrong key prefix",
            )),
        }
    }
}

impl fmt::Display for KeyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.write_str(match self {
            Self::Ed25519 => "ed25519",
            Self::Secp256k1 => "secp256k1",
        })
    }
}

impl FromStr for KeyType {
    type Err = DecodeBs58Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let lowercase_key_type = value.to_ascii_lowercase();
        match lowercase_key_type.as_str() {
            "ed25519" => Ok(Self::Ed25519),
            "secp256k1" => Ok(Self::Secp256k1),
            _ => Err(Self::Err::BadData(value.to_string())),
        }
    }
}

fn split_key_type_data(value: &str) -> Result<(KeyType, &str), DecodeBs58Error> {
    if let Some(idx) = value.find(':') {
        let (prefix, key_data) = value.split_at(idx);
        Ok((KeyType::from_str(prefix)?, &key_data[1..]))
    } else {
        // If there is no prefix then we Default to ED25519.
        Ok((KeyType::Ed25519, value))
    }
}

/// Helper struct which provides Display implementation for bytes slice
/// encoding them using base58.
// TODO(mina86): Get rid of it once bs58 has this feature.  There’s currently PR
// for that: https://github.com/Nullus157/bs58-rs/pull/97
struct Bs58<'a>(&'a [u8]);

impl<'a> fmt::Display for Bs58<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        debug_assert!(self.0.len() <= 65);
        // The largest buffer we’re ever encoding is 65-byte long.  Base58
        // increases size of the value by less than 40%.  96-byte buffer is
        // therefore enough to fit the largest value we’re ever encoding.
        let mut buf = [0u8; 96];
        let len = bs58::encode(self.0).onto(&mut buf[..]).unwrap();
        let output = &buf[..len];
        // SAFETY: we know that alphabet can only include ASCII characters
        // thus our result is an ASCII string.
        fmt.write_str(unsafe { crate::str::from_utf8_unchecked(output) })
    }
}

/// Helper which decodes fixed-length base58-encoded data.
///
/// If the encoded string decodes into a buffer of different length than `N`,
/// returns error.  Similarly returns error if decoding fails.
fn decode_bs58<const N: usize>(encoded: &str) -> Result<[u8; N], DecodeBs58Error> {
    let mut buffer = [0u8; N];
    decode_bs58_impl(&mut buffer[..], encoded)?;
    Ok(buffer)
}

fn decode_bs58_impl(dst: &mut [u8], encoded: &str) -> Result<(), DecodeBs58Error> {
    let expected = dst.len();
    match bs58::decode(encoded).onto(dst) {
        Ok(received) if received == expected => Ok(()),
        Ok(received) => Err(DecodeBs58Error::BadLength { expected, received }),
        Err(bs58::decode::Error::BufferTooSmall) => Err(DecodeBs58Error::BadLength {
            expected,
            received: expected.saturating_add(1),
        }),
        Err(err) => Err(DecodeBs58Error::BadData(err.to_string())),
    }
}

#[derive(Debug)]
pub enum DecodeBs58Error {
    BadLength { expected: usize, received: usize },
    BadData(String),
}

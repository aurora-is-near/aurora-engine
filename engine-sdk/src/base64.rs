use aurora_engine_types::{String, Vec};
pub use base64::DecodeError;
use base64::Engine;

/// Encode arbitrary octets as base64 using the standard `base64::Engine`.
pub fn encode<T: AsRef<[u8]>>(input: T) -> String {
    base64::engine::general_purpose::STANDARD.encode(input)
}

/// Decode from string reference as octets using the standard `base64::Engine`.
pub fn decode<T: AsRef<[u8]>>(input: T) -> Result<Vec<u8>, DecodeError> {
    base64::engine::general_purpose::STANDARD.decode(input)
}

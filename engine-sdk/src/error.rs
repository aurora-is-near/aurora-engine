#[derive(Debug)]
pub struct BorshDeserializeError;

impl AsRef<[u8]> for BorshDeserializeError {
    fn as_ref(&self) -> &[u8] {
        b"ERR_ARG_PARSE"
    }
}

#[derive(Debug)]
pub struct IncorrectInputLength;

impl AsRef<[u8]> for IncorrectInputLength {
    fn as_ref(&self) -> &[u8] {
        b"ERR_INCORRECT_INPUT_LENGTH"
    }
}

#[derive(Debug)]
pub enum ReadU32Error {
    InvalidU32,
    MissingValue,
}

impl AsRef<[u8]> for ReadU32Error {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::InvalidU32 => b"ERR_NOT_U32",
            Self::MissingValue => b"ERR_U32_NOT_FOUND",
        }
    }
}

#[derive(Debug)]
pub enum ReadU64Error {
    InvalidU64,
    MissingValue,
}

impl AsRef<[u8]> for ReadU64Error {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::InvalidU64 => b"ERR_NOT_U64",
            Self::MissingValue => b"ERR_U64_NOT_FOUND",
        }
    }
}

#[derive(Debug)]
pub enum ReadU256Error {
    InvalidU256,
    MissingValue,
}

impl AsRef<[u8]> for ReadU256Error {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::InvalidU256 => b"ERR_NOT_U256",
            Self::MissingValue => b"ERR_U256_NOT_FOUND",
        }
    }
}

#[derive(Debug)]
pub struct PrivateCallError;

impl AsRef<[u8]> for PrivateCallError {
    fn as_ref(&self) -> &[u8] {
        b"ERR_PRIVATE_CALL"
    }
}

#[derive(Debug)]
pub struct OneYoctoAttachError;

impl AsRef<[u8]> for OneYoctoAttachError {
    fn as_ref(&self) -> &[u8] {
        b"ERR_1YOCTO_ATTACH"
    }
}

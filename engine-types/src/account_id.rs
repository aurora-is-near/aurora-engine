//! Guarantees all properly constructed AccountId's are valid for the NEAR network.
//!
//! Inpired by: https://github.com/near/nearcore/tree/master/core/account-id

use crate::{fmt, str::FromStr, Box, String, TryFrom};
use borsh::{BorshDeserialize, BorshSerialize};

pub const MIN_ACCOUNT_ID_LEN: usize = 2;
pub const MAX_ACCOUNT_ID_LEN: usize = 64;

/// Account identifier.
///
/// This guarantees all properly constructed AccountId's are valid for the NEAR network.
#[derive(BorshSerialize, BorshDeserialize, Eq, Ord, Hash, Clone, Debug, PartialEq, PartialOrd)]
pub struct AccountId(Box<str>);

impl AccountId {
    pub fn new(account_id: &str) -> Result<Self, ParseAccountError> {
        Self::validate(account_id)?;
        Ok(Self(account_id.into()))
    }

    pub fn validate(account_id: &str) -> Result<(), ParseAccountError> {
        if account_id.len() < MIN_ACCOUNT_ID_LEN {
            Err(ParseAccountError::TooShort)
        } else if account_id.len() > MAX_ACCOUNT_ID_LEN {
            Err(ParseAccountError::TooLong)
        } else {
            // Adapted from https://github.com/near/near-sdk-rs/blob/fd7d4f82d0dfd15f824a1cf110e552e940ea9073/near-sdk/src/environment/env.rs#L819

            // NOTE: We don't want to use Regex here, because it requires extra time to compile it.
            // The valid account ID regex is /^(([a-z\d]+[-_])*[a-z\d]+\.)*([a-z\d]+[-_])*[a-z\d]+$/
            // Instead the implementation is based on the previous character checks.

            // We can safely assume that last char was a separator.
            let mut last_char_is_separator = true;

            for c in account_id.bytes() {
                let current_char_is_separator = match c {
                    b'a'..=b'z' | b'0'..=b'9' => false,
                    b'-' | b'_' | b'.' => true,
                    _ => {
                        return Err(ParseAccountError::Invalid);
                    }
                };
                if current_char_is_separator && last_char_is_separator {
                    return Err(ParseAccountError::Invalid);
                }
                last_char_is_separator = current_char_is_separator;
            }

            (!last_char_is_separator)
                .then(|| ())
                .ok_or(ParseAccountError::Invalid)
        }
    }
}

impl TryFrom<String> for AccountId {
    type Error = ParseAccountError;

    fn try_from(account_id: String) -> Result<Self, Self::Error> {
        AccountId::new(&account_id)
    }
}

impl FromStr for AccountId {
    type Err = ParseAccountError;

    fn from_str(account_id: &str) -> Result<Self, Self::Err> {
        Self::validate(account_id)?;
        Ok(Self(account_id.into()))
    }
}

impl From<AccountId> for String {
    fn from(account_id: AccountId) -> Self {
        account_id.0.into_string()
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<AccountId> for Box<str> {
    fn from(value: AccountId) -> Box<str> {
        value.0
    }
}

impl<T: ?Sized> AsRef<T> for AccountId
where
    Box<str>: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.0.as_ref()
    }
}

/// A list of errors that occur when parsing an invalid Account ID.
#[derive(Eq, Hash, Clone, Debug, PartialEq)]
pub enum ParseAccountError {
    TooLong,
    TooShort,
    Invalid,
}

impl AsRef<[u8]> for ParseAccountError {
    fn as_ref(&self) -> &[u8] {
        match self {
            ParseAccountError::TooLong => b"ERR_ACCOUNT_ID_TO_LONG",
            ParseAccountError::TooShort => b"ERR_ACCOUNT_ID_TO_SHORT",
            ParseAccountError::Invalid => b"ERR_ACCOUNT_ID_TO_INVALID",
        }
    }
}

impl fmt::Display for ParseAccountError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = String::from_utf8(self.as_ref().to_vec()).unwrap();
        write!(f, "{}", msg)
    }
}

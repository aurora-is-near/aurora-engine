//! Guarantees all properly constructed AccountId's are valid for the NEAR network.
//!
//! Inpired by: https://github.com/near/nearcore/tree/master/core/account-id

use crate::{fmt, str, str::FromStr, Box, String, Vec};
use borsh::{BorshDeserialize, BorshSerialize};

pub const MIN_ACCOUNT_ID_LEN: usize = 2;
pub const MAX_ACCOUNT_ID_LEN: usize = 64;

/// Account identifier.
///
/// This guarantees all properly constructed AccountId's are valid for the NEAR network.
#[derive(
    BorshSerialize, BorshDeserialize, Default, Eq, Ord, Hash, Clone, Debug, PartialEq, PartialOrd,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AccountId(Box<str>);

impl AccountId {
    pub fn new(account_id: &str) -> Result<Self, ParseAccountError> {
        Self::validate(account_id)?;
        Ok(Self(account_id.into()))
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.as_ref().as_bytes()
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
                .then_some(())
                .ok_or(ParseAccountError::Invalid)
        }
    }

    pub fn is_top_level_account_id(&self) -> bool {
        self.0.len() >= MIN_ACCOUNT_ID_LEN
            && self.0.len() <= MAX_ACCOUNT_ID_LEN
            && self.as_ref() != "system"
            && !self.as_ref().contains('.')
    }

    /// Returns true if the signer_id can create a direct sub-account with the given account Id.
    pub fn is_sub_account_of(&self, parent_account_id: &AccountId) -> bool {
        if parent_account_id.0.len() >= self.0.len() {
            return false;
        }
        // Will not panic, since valid account id is utf-8 only and the length is checked above.
        // e.g. when `near` creates `aa.near`, it splits into `aa.` and `near`
        let (prefix, suffix) = self.0.split_at(self.0.len() - parent_account_id.0.len());

        prefix.find('.') == Some(prefix.len() - 1) && suffix == parent_account_id.as_ref()
    }
}

impl TryFrom<String> for AccountId {
    type Error = ParseAccountError;

    fn try_from(account_id: String) -> Result<Self, Self::Error> {
        AccountId::new(&account_id)
    }
}

impl TryFrom<&[u8]> for AccountId {
    type Error = ParseAccountError;

    fn try_from(account_id: &[u8]) -> Result<Self, Self::Error> {
        let account_id = str::from_utf8(account_id).map_err(|_| ParseAccountError::Invalid)?;
        AccountId::new(account_id)
    }
}

impl TryFrom<Vec<u8>> for AccountId {
    type Error = ParseAccountError;

    fn try_from(account_id: Vec<u8>) -> Result<Self, Self::Error> {
        AccountId::try_from(&account_id[..])
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

impl From<AccountId> for Vec<u8> {
    fn from(account_id: AccountId) -> Vec<u8> {
        account_id.as_bytes().to_vec()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn is_implicit(account_id: &str) -> bool {
        account_id.len() == 64
            && account_id
                .as_bytes()
                .iter()
                .all(|b| matches!(b, b'a'..=b'f' | b'0'..=b'9'))
    }

    pub const OK_ACCOUNT_IDS: [&str; 24] = [
        "aa",
        "a-a",
        "a-aa",
        "100",
        "0o",
        "com",
        "near",
        "bowen",
        "b-o_w_e-n",
        "b.owen",
        "bro.wen",
        "a.ha",
        "a.b-a.ra",
        "system",
        "over.9000",
        "google.com",
        "illia.cheapaccounts.near",
        "0o0ooo00oo00o",
        "alex-skidanov",
        "10-4.8-2",
        "b-o_w_e-n",
        "no_lols",
        "0123456789012345678901234567890123456789012345678901234567890123",
        // Valid, but can't be created
        "near.a",
    ];

    pub const BAD_ACCOUNT_IDS: [&str; 24] = [
        "a",
        "A",
        "Abc",
        "-near",
        "near-",
        "-near-",
        "near.",
        ".near",
        "near@",
        "@near",
        "неар",
        "@@@@@",
        "0__0",
        "0_-_0",
        "0_-_0",
        "..",
        "a..near",
        "nEar",
        "_bowen",
        "hello world",
        "abcdefghijklmnopqrstuvwxyz.abcdefghijklmnopqrstuvwxyz.abcdefghijklmnopqrstuvwxyz",
        "01234567890123456789012345678901234567890123456789012345678901234",
        // `@` separators are banned now
        "some-complex-address@gmail.com",
        "sub.buy_d1gitz@atata@b0-rg.c_0_m",
    ];

    #[test]
    fn test_is_valid_account_id() {
        for account_id in OK_ACCOUNT_IDS.iter().cloned() {
            if let Err(err) = AccountId::validate(account_id) {
                panic!("Valid account id {:?} marked invalid: {}", account_id, err);
            }
        }

        for account_id in BAD_ACCOUNT_IDS.iter().cloned() {
            if let Ok(_) = AccountId::validate(account_id) {
                panic!("Valid account id {:?} marked valid", account_id);
            }
        }
    }

    #[test]
    fn test_is_valid_top_level_account_id() {
        let ok_top_level_account_ids = &[
            "aa",
            "a-a",
            "a-aa",
            "100",
            "0o",
            "com",
            "near",
            "bowen",
            "b-o_w_e-n",
            "0o0ooo00oo00o",
            "alex-skidanov",
            "b-o_w_e-n",
            "no_lols",
            "0123456789012345678901234567890123456789012345678901234567890123",
        ];
        for account_id in ok_top_level_account_ids {
            assert!(
                account_id
                    .parse::<AccountId>()
                    .map_or(false, |account_id| account_id.is_top_level_account_id()),
                "Valid top level account id {:?} marked invalid",
                account_id
            );
        }

        let bad_top_level_account_ids = &[
            "near.a",
            "b.owen",
            "bro.wen",
            "a.ha",
            "a.b-a.ra",
            "some-complex-address@gmail.com",
            "sub.buy_d1gitz@atata@b0-rg.c_0_m",
            "over.9000",
            "google.com",
            "illia.cheapaccounts.near",
            "10-4.8-2",
            "a",
            "A",
            "Abc",
            "-near",
            "near-",
            "-near-",
            "near.",
            ".near",
            "near@",
            "@near",
            "неар",
            "@@@@@",
            "0__0",
            "0_-_0",
            "0_-_0",
            "..",
            "a..near",
            "nEar",
            "_bowen",
            "hello world",
            "abcdefghijklmnopqrstuvwxyz.abcdefghijklmnopqrstuvwxyz.abcdefghijklmnopqrstuvwxyz",
            "01234567890123456789012345678901234567890123456789012345678901234",
            // Valid regex and length, but reserved
            "system",
        ];
        for account_id in bad_top_level_account_ids {
            assert!(
                !account_id
                    .parse::<AccountId>()
                    .map_or(false, |account_id| account_id.is_top_level_account_id()),
                "Invalid top level account id {:?} marked valid",
                account_id
            );
        }
    }

    #[test]
    fn test_is_valid_sub_account_id() {
        let ok_pairs = &[
            ("test", "a.test"),
            ("test-me", "abc.test-me"),
            ("gmail.com", "abc.gmail.com"),
            ("gmail.com", "abc-lol.gmail.com"),
            ("gmail.com", "abc_lol.gmail.com"),
            ("gmail.com", "bro-abc_lol.gmail.com"),
            ("g0", "0g.g0"),
            ("1g", "1g.1g"),
            ("5-3", "4_2.5-3"),
        ];
        for (signer_id, sub_account_id) in ok_pairs {
            assert!(
                matches!(
                    (signer_id.parse::<AccountId>(), sub_account_id.parse::<AccountId>()),
                    (Ok(signer_id), Ok(sub_account_id)) if sub_account_id.is_sub_account_of(&signer_id)
                ),
                "Failed to create sub-account {:?} by account {:?}",
                sub_account_id,
                signer_id
            );
        }

        let bad_pairs = &[
            ("test", ".test"),
            ("test", "test"),
            ("test", "a1.a.test"),
            ("test", "est"),
            ("test", ""),
            ("test", "st"),
            ("test5", "ббб"),
            ("test", "a-test"),
            ("test", "etest"),
            ("test", "a.etest"),
            ("test", "retest"),
            ("test-me", "abc-.test-me"),
            ("test-me", "Abc.test-me"),
            ("test-me", "-abc.test-me"),
            ("test-me", "a--c.test-me"),
            ("test-me", "a_-c.test-me"),
            ("test-me", "a-_c.test-me"),
            ("test-me", "_abc.test-me"),
            ("test-me", "abc_.test-me"),
            ("test-me", "..test-me"),
            ("test-me", "a..test-me"),
            ("gmail.com", "a.abc@gmail.com"),
            ("gmail.com", ".abc@gmail.com"),
            ("gmail.com", ".abc@gmail@com"),
            ("gmail.com", "abc@gmail@com"),
            ("test", "a@test"),
            ("test_me", "abc@test_me"),
            ("gmail.com", "abc@gmail.com"),
            ("gmail@com", "abc.gmail@com"),
            ("gmail.com", "abc-lol@gmail.com"),
            ("gmail@com", "abc_lol.gmail@com"),
            ("gmail@com", "bro-abc_lol.gmail@com"),
            (
                "gmail.com",
                "123456789012345678901234567890123456789012345678901234567890@gmail.com",
            ),
            (
                "123456789012345678901234567890123456789012345678901234567890",
                "1234567890.123456789012345678901234567890123456789012345678901234567890",
            ),
            ("aa", "ъ@aa"),
            ("aa", "ъ.aa"),
        ];
        for (signer_id, sub_account_id) in bad_pairs {
            assert!(
                !matches!(
                    (signer_id.parse::<AccountId>(), sub_account_id.parse::<AccountId>()),
                    (Ok(signer_id), Ok(sub_account_id)) if sub_account_id.is_sub_account_of(&signer_id)
                ),
                "Invalid sub-account {:?} created by account {:?}",
                sub_account_id,
                signer_id
            );
        }
    }

    #[test]
    fn test_is_account_id_64_len_hex() {
        let valid_64_len_hex_account_ids = &[
            "0000000000000000000000000000000000000000000000000000000000000000",
            "6174617461746174617461746174617461746174617461746174617461746174",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            "20782e20662e64666420482123494b6b6c677573646b6c66676a646b6c736667",
        ];
        for valid_account_id in valid_64_len_hex_account_ids {
            assert!(
                matches!(
                    valid_account_id.parse::<AccountId>(),
                    Ok(account_id) if is_implicit(account_id.as_ref())
                ),
                "Account ID {} should be valid 64-len hex",
                valid_account_id
            );
            assert!(
                is_implicit(valid_account_id),
                "Account ID {} should be valid 64-len hex",
                valid_account_id
            );
        }

        let invalid_64_len_hex_account_ids = &[
            "000000000000000000000000000000000000000000000000000000000000000",
            "6.74617461746174617461746174617461746174617461746174617461746174",
            "012-456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "fffff_ffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            "oooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooo",
            "00000000000000000000000000000000000000000000000000000000000000",
        ];
        for invalid_account_id in invalid_64_len_hex_account_ids {
            assert!(
                !matches!(
                    invalid_account_id.parse::<AccountId>(),
                    Ok(account_id) if is_implicit(account_id.as_ref())
                ),
                "Account ID {} should be invalid 64-len hex",
                invalid_account_id
            );
            assert!(
                !is_implicit(invalid_account_id),
                "Account ID {} should be invalid 64-len hex",
                invalid_account_id
            );
        }
    }
}

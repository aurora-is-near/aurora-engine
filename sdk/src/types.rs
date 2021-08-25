use crate::*;
use prelude::Address;

#[cfg(feature = "contract")]
#[inline]
pub fn keccak(data: &[u8]) -> H256 {
    crate::keccak(data)
}

#[cfg(not(feature = "contract"))]
#[inline]
pub fn keccak(data: &[u8]) -> H256 {
    H256::from_slice(Keccak256::digest(data).as_slice())
}

#[allow(dead_code)]
pub fn near_account_to_evm_address(addr: &[u8]) -> Address {
    Address::from_slice(&keccak(addr)[12..])
}

#[cfg(feature = "contract")]
pub trait ExpectUtf8<T> {
    fn expect_utf8(self, message: &[u8]) -> T;
}

#[cfg(feature = "contract")]
impl<T> ExpectUtf8<T> for Option<T> {
    fn expect_utf8(self, message: &[u8]) -> T {
        match self {
            Some(t) => t,
            None => panic_utf8(message),
        }
    }
}

#[cfg(feature = "contract")]
impl<T, E> ExpectUtf8<T> for core::result::Result<T, E> {
    fn expect_utf8(self, message: &[u8]) -> T {
        match self {
            Ok(t) => t,
            Err(_) => panic_utf8(message),
        }
    }
}

pub trait SdkExpect<T> {
    fn sdk_expect(self, msg: &str) -> T;
}

impl<T> SdkExpect<T> for Option<T> {
    fn sdk_expect(self, msg: &str) -> T {
        match self {
            Some(t) => t,
            None => panic_utf8(msg.as_ref()),
        }
    }
}

impl<T, E> SdkExpect<T> for core::result::Result<T, E> {
    fn sdk_expect(self, msg: &str) -> T {
        match self {
            Ok(t) => t,
            Err(_) => panic_utf8(msg.as_ref()),
        }
    }
}

pub trait SdkUnwrap<T> {
    fn sdk_unwrap(self) -> T;
}

impl<T> SdkUnwrap<T> for Option<T> {
    fn sdk_unwrap(self) -> T {
        match self {
            Some(t) => t,
            None => panic_utf8("ERR_UNWRAP".as_bytes()),
        }
    }
}

impl<T, E: AsRef<[u8]>> SdkUnwrap<T> for core::result::Result<T, E> {
    fn sdk_unwrap(self) -> T {
        match self {
            Ok(t) => t,
            Err(e) => panic_utf8(e.as_ref()),
        }
    }
}

pub(crate) trait SdkProcess<T> {
    fn sdk_process(self);
}

impl<T: AsRef<[u8]>, E: AsRef<[u8]>> SdkProcess<T> for Result<T, E> {
    fn sdk_process(self) {
        match self {
            Ok(r) => return_output(r.as_ref()),
            Err(e) => panic_utf8(e.as_ref()),
        }
    }
}

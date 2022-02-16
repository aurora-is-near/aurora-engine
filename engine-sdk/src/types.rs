#[cfg(feature = "contract")]
use crate::io::IO;
use crate::prelude::{Address, H256};

#[cfg(not(feature = "contract"))]
use sha3::{Digest, Keccak256};

#[cfg(feature = "contract")]
#[inline]
pub fn keccak(input: &[u8]) -> H256 {
    unsafe {
        super::exports::keccak256(input.len() as u64, input.as_ptr() as u64, 1);
        let bytes = H256::zero();
        super::exports::read_register(1, bytes.0.as_ptr() as *const u64 as u64);
        bytes
    }
}

#[cfg(not(feature = "contract"))]
#[inline]
pub fn keccak(data: &[u8]) -> H256 {
    H256::from_slice(Keccak256::digest(data).as_slice())
}

pub fn near_account_to_evm_address(addr: &[u8]) -> Address {
    Address::try_from_slice(&keccak(addr)[12..]).unwrap()
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
            None => crate::panic_utf8(message),
        }
    }
}

#[cfg(feature = "contract")]
impl<T, E> ExpectUtf8<T> for core::result::Result<T, E> {
    fn expect_utf8(self, message: &[u8]) -> T {
        match self {
            Ok(t) => t,
            Err(_) => crate::panic_utf8(message),
        }
    }
}

#[cfg(feature = "contract")]
pub trait SdkExpect<T> {
    fn sdk_expect(self, msg: &str) -> T;
}

#[cfg(feature = "contract")]
impl<T> SdkExpect<T> for Option<T> {
    fn sdk_expect(self, msg: &str) -> T {
        match self {
            Some(t) => t,
            None => crate::panic_utf8(msg.as_ref()),
        }
    }
}

#[cfg(feature = "contract")]
impl<T, E> SdkExpect<T> for core::result::Result<T, E> {
    fn sdk_expect(self, msg: &str) -> T {
        match self {
            Ok(t) => t,
            Err(_) => crate::panic_utf8(msg.as_ref()),
        }
    }
}

#[cfg(feature = "contract")]
pub trait SdkUnwrap<T> {
    fn sdk_unwrap(self) -> T;
}

#[cfg(feature = "contract")]
impl<T> SdkUnwrap<T> for Option<T> {
    fn sdk_unwrap(self) -> T {
        match self {
            Some(t) => t,
            None => crate::panic_utf8("ERR_UNWRAP".as_bytes()),
        }
    }
}

#[cfg(feature = "contract")]
impl<T, E: AsRef<[u8]>> SdkUnwrap<T> for core::result::Result<T, E> {
    fn sdk_unwrap(self) -> T {
        match self {
            Ok(t) => t,
            Err(e) => crate::panic_utf8(e.as_ref()),
        }
    }
}

#[cfg(feature = "contract")]
pub trait SdkProcess<T> {
    fn sdk_process(self);
}

#[cfg(feature = "contract")]
impl<T: AsRef<[u8]>, E: AsRef<[u8]>> SdkProcess<T> for Result<T, E> {
    fn sdk_process(self) {
        match self {
            Ok(r) => crate::near_runtime::Runtime.return_output(r.as_ref()),
            Err(e) => crate::panic_utf8(e.as_ref()),
        }
    }
}

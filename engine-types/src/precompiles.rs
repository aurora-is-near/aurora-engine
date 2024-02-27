#![allow(clippy::unreadable_literal)]
use crate::types::{make_address, Address};
use bitflags::bitflags;
use borsh::{self, BorshDeserialize, BorshSerialize};

/// Exit to Ethereum precompile address
///
/// Address: `0xb0bd02f6a392af548bdf1cfaee5dfa0eefcc8eab`
/// This address is computed as: `&keccak("exitToEthereum")[12..]`
pub const EXIT_TO_ETHEREUM_ADDRESS: Address =
    make_address(0xb0bd02f6, 0xa392af548bdf1cfaee5dfa0eefcc8eab);

/// Exit to NEAR precompile address
///
/// Address: `0xe9217bc70b7ed1f598ddd3199e80b093fa71124f`
/// This address is computed as: `&keccak("exitToNear")[12..]`
pub const EXIT_TO_NEAR_ADDRESS: Address =
    make_address(0xe9217bc7, 0x0b7ed1f598ddd3199e80b093fa71124f);

bitflags! {
    /// Wraps unsigned integer where each bit identifies a different precompile.
    #[derive(BorshSerialize, BorshDeserialize, Default)]
    pub struct PrecompileFlags: u32 {
        const EXIT_TO_NEAR        = 0b01;
        const EXIT_TO_ETHEREUM    = 0b10;
    }
}

impl PrecompileFlags {
    #[must_use]
    pub fn from_address(address: &Address) -> Option<Self> {
        Some(if address == &EXIT_TO_ETHEREUM_ADDRESS {
            Self::EXIT_TO_ETHEREUM
        } else if address == &EXIT_TO_NEAR_ADDRESS {
            Self::EXIT_TO_NEAR
        } else {
            return None;
        })
    }

    /// Checks if the precompile belonging to the `address` is marked as paused.
    #[must_use]
    pub fn is_paused_by_address(&self, address: &Address) -> bool {
        Self::from_address(address).map_or(false, |precompile_flag| self.contains(precompile_flag))
    }
}

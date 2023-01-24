use crate::prelude::{BorshDeserialize, BorshSerialize};
use aurora_engine_sdk::io::{StorageIntermediate, IO};
use aurora_engine_types::storage;
use aurora_engine_types::storage::KeyPrefix;
use aurora_engine_types::types::Address;

mod errors {
    pub const ERR_DESERIALIZE_GAS_TOKEN: &str = "ERR_DESERIALIZE_GAS_TOKEN";
}

#[derive(BorshSerialize, BorshDeserialize)]
/// Used to select which gas token to pay in.
pub enum GasToken {
    /// Gas is paid in Ether.
    Base,
    /// Gas is paid in a ERC-20 compatible token.
    Erc20(Address),
}

impl GasToken {
    // TODO: wait for use of this
    // fn into_address(self) -> Address {
    //     use GasToken::*;
    //     match self {
    //         Base => Address::from_array([0u8; 20]),
    //         Erc20(addr) => addr,
    //     }
    // }

    pub(crate) fn from_address(address: Address) -> GasToken {
        use GasToken::*;
        if address == Address::zero() {
            Base
        } else {
            Erc20(address)
        }
    }
}

/// Sets the gas token for a given address and returns the old value, if any.
pub fn set_gas_token<I: IO>(io: &mut I, address: Address, gas_token: GasToken) -> Option<GasToken> {
    let key = storage::bytes_to_key(KeyPrefix::GasToken, address.as_bytes());
    io.write_storage_borsh(&key, &gas_token)
        .map(|v| v.to_value().expect(errors::ERR_DESERIALIZE_GAS_TOKEN))
}

/// Gets the gas token set for a given address, if any.
pub fn get_gas_token<I: IO>(io: &I, address: &Address) -> Option<GasToken> {
    let key = storage::bytes_to_key(KeyPrefix::GasToken, address.as_bytes());
    io.read_storage(&key)
        .map(|v| v.to_value().expect(errors::ERR_DESERIALIZE_GAS_TOKEN))
}

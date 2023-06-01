use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::borsh::{self, BorshDeserialize, BorshSerialize};
use aurora_engine_types::types::{Address, Wei};

use crate::silo::whitelist::WhitelistKind;

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct SiloParamsArgs {
    pub fixed_gas_cost: Wei,
    pub erc20_fallback_address: Address,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(
    feature = "impl-serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(untagged)
)]
pub enum WhitelistArgs {
    WhitelistAddressArgs(WhitelistAddressArgs),
    WhitelistAccountArgs(WhitelistAccountArgs),
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WhitelistAddressArgs {
    pub kind: WhitelistKind,
    pub address: Address,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WhitelistAccountArgs {
    pub kind: WhitelistKind,
    pub account_id: AccountId,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct WhitelistStatusArgs {
    pub kind: WhitelistKind,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct WhitelistKindArgs {
    pub kind: WhitelistKind,
}

#[test]
fn test_account_whitelist_serialize() {
    let args = WhitelistArgs::WhitelistAccountArgs(WhitelistAccountArgs {
        account_id: "aurora".parse().unwrap(),
        kind: WhitelistKind::Admin,
    });
    let bytes = args.try_to_vec().unwrap();
    let args = WhitelistArgs::try_from_slice(&bytes).unwrap();

    assert_eq!(
        args,
        WhitelistArgs::WhitelistAccountArgs(WhitelistAccountArgs {
            account_id: "aurora".parse().unwrap(),
            kind: WhitelistKind::Admin,
        })
    );
}

#[test]
fn test_address_whitelist_serialize() {
    let address = Address::decode("096DE9C2B8A5B8c22cEe3289B101f6960d68E51E").unwrap();
    let args = WhitelistArgs::WhitelistAddressArgs(WhitelistAddressArgs {
        address,
        kind: WhitelistKind::EvmAdmin,
    });
    let bytes = args.try_to_vec().unwrap();
    let args = WhitelistArgs::try_from_slice(&bytes).unwrap();

    assert_eq!(
        args,
        WhitelistArgs::WhitelistAddressArgs(WhitelistAddressArgs {
            address,
            kind: WhitelistKind::EvmAdmin,
        })
    );
}

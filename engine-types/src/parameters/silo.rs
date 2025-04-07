use crate::account_id::AccountId;
use crate::borsh::{self, BorshDeserialize, BorshSerialize};
use crate::types::{Address, EthGas};

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct FixedGasArgs {
    pub fixed_gas: Option<EthGas>,
}

#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Erc20FallbackAddressArgs {
    pub address: Option<Address>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct SiloParamsArgs {
    /// Fixed amount of gas per transaction.
    pub fixed_gas: EthGas,
    /// EVM address, which is used for withdrawing ERC-20 base tokens in case
    /// a recipient of the tokens is not in the silo white list.
    /// Note: the logic described above works only if the fallback address
    /// is set by `set_silo_params` function. In other words, in Silo mode.
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[cfg_attr(feature = "impl-serde", derive(serde::Serialize, serde::Deserialize))]
#[borsh(use_discriminant = false)]
pub enum WhitelistKind {
    /// The whitelist of this type is for storing NEAR accounts. Accounts stored in this whitelist
    /// have an admin role. The admin role allows to add new admins and add new entities
    /// (`AccountId` and `Address`) to whitelists. Also, this role allows to deploy of EVM code
    /// and submit transactions.
    Admin = 0x0,
    /// The whitelist of this type is for storing EVM addresses. Addresses included in this
    /// whitelist can deploy EVM code.
    EvmAdmin = 0x1,
    /// The whitelist of this type is for storing NEAR accounts. Accounts included in this
    /// whitelist can submit transactions.
    Account = 0x2,
    /// The whitelist of this type is for storing EVM addresses. Addresses included in this
    /// whitelist can submit transactions.
    Address = 0x3,
}

impl From<WhitelistKind> for u8 {
    fn from(list: WhitelistKind) -> Self {
        match list {
            WhitelistKind::Admin => 0x0,
            WhitelistKind::EvmAdmin => 0x1,
            WhitelistKind::Account => 0x2,
            WhitelistKind::Address => 0x3,
        }
    }
}

#[test]
fn test_account_whitelist_serialize() {
    let args = WhitelistArgs::WhitelistAccountArgs(WhitelistAccountArgs {
        account_id: "aurora".parse().unwrap(),
        kind: WhitelistKind::Admin,
    });
    let bytes = borsh::to_vec(&args).unwrap();
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
    let bytes = borsh::to_vec(&args).unwrap();
    let args = WhitelistArgs::try_from_slice(&bytes).unwrap();

    assert_eq!(
        args,
        WhitelistArgs::WhitelistAddressArgs(WhitelistAddressArgs {
            address,
            kind: WhitelistKind::EvmAdmin,
        })
    );
}

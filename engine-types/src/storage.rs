use crate::{types::Address, Vec, H256};
use borsh::{BorshDeserialize, BorshSerialize};

// NOTE: We start at 0x7 as our initial value as our original storage was not
// version prefixed and ended as 0x6.
pub enum VersionPrefix {
    V1 = 0x7,
}

impl From<VersionPrefix> for u8 {
    fn from(v: VersionPrefix) -> Self {
        match v {
            VersionPrefix::V1 => 0x7,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, BorshSerialize, BorshDeserialize)]
#[borsh(use_discriminant = false)]
pub enum KeyPrefix {
    Config = 0x0,
    Nonce = 0x1,
    Balance = 0x2,
    Code = 0x3,
    Storage = 0x4,
    RelayerEvmAddressMap = 0x5,
    EthConnector = 0x6,
    Generation = 0x7,
    Nep141Erc20Map = 0x8,
    Erc20Nep141Map = 0x9,
    CrossContractCall = 0xa,
    RelayerFunctionCallKey = 0xb,
    Hashchain = 0xc,
    Silo = 0xd,
    Whitelist = 0xe,
    CodeHash = 0xf,
}

impl From<KeyPrefix> for u8 {
    fn from(k: KeyPrefix) -> Self {
        match k {
            KeyPrefix::Config => 0x0,
            KeyPrefix::Nonce => 0x1,
            KeyPrefix::Balance => 0x2,
            KeyPrefix::Code => 0x3,
            KeyPrefix::Storage => 0x4,
            KeyPrefix::RelayerEvmAddressMap => 0x5,
            KeyPrefix::EthConnector => 0x6,
            KeyPrefix::Generation => 0x7,
            KeyPrefix::Nep141Erc20Map => 0x8,
            KeyPrefix::Erc20Nep141Map => 0x9,
            KeyPrefix::CrossContractCall => 0xa,
            KeyPrefix::RelayerFunctionCallKey => 0xb,
            KeyPrefix::Hashchain => 0xc,
            KeyPrefix::Silo => 0xd,
            KeyPrefix::Whitelist => 0xe,
            KeyPrefix::CodeHash => 0xf,
        }
    }
}

/// Enum used to differentiate different storage keys used by eth-connector
#[derive(Clone, Copy, BorshSerialize, BorshDeserialize)]
#[borsh(use_discriminant = false)]
pub enum EthConnectorStorageId {
    Contract = 0x0,
    FungibleToken = 0x1,
    UsedEvent = 0x2,
    PausedMask = 0x3,
    StatisticsAuroraAccountsCounter = 0x4,
    FungibleTokenMetadata = 0x5,
    EthConnectorAccount = 0x6,
    WithdrawSerializationType = 0x7,
}

impl From<EthConnectorStorageId> for u8 {
    fn from(id: EthConnectorStorageId) -> Self {
        match id {
            EthConnectorStorageId::Contract => 0x0,
            EthConnectorStorageId::FungibleToken => 0x1,
            EthConnectorStorageId::UsedEvent => 0x2,
            EthConnectorStorageId::PausedMask => 0x3,
            EthConnectorStorageId::StatisticsAuroraAccountsCounter => 0x4,
            EthConnectorStorageId::FungibleTokenMetadata => 0x5,
            EthConnectorStorageId::EthConnectorAccount => 0x6,
            EthConnectorStorageId::WithdrawSerializationType => 0x7,
        }
    }
}

/// We can't use const generic over Enum, but we can do it over integral type
pub type KeyPrefixU8 = u8;

// TODO: Derive From<u8> using macro to avoid missing new arguments in the future
impl From<KeyPrefixU8> for KeyPrefix {
    fn from(value: KeyPrefixU8) -> Self {
        match value {
            0x0 => Self::Config,
            0x1 => Self::Nonce,
            0x2 => Self::Balance,
            0x3 => Self::Code,
            0x4 => Self::Storage,
            0x5 => Self::RelayerEvmAddressMap,
            0x6 => Self::EthConnector,
            0x7 => Self::Generation,
            0x8 => Self::Nep141Erc20Map,
            0x9 => Self::Erc20Nep141Map,
            0xa => Self::CrossContractCall,
            0xb => Self::RelayerFunctionCallKey,
            0xc => Self::Hashchain,
            0xd => Self::Silo,
            0xe => Self::Whitelist,
            0xf => Self::CodeHash,
            _ => unreachable!("Unknown key prefix"),
        }
    }
}

#[allow(dead_code)]
#[must_use]
pub fn bytes_to_key(prefix: KeyPrefix, bytes: &[u8]) -> Vec<u8> {
    [&[u8::from(VersionPrefix::V1)], &[u8::from(prefix)], bytes].concat()
}

#[allow(dead_code)]
#[must_use]
pub fn address_to_key(prefix: KeyPrefix, address: &Address) -> [u8; 22] {
    let mut result = [0u8; 22];
    result[0] = u8::from(VersionPrefix::V1);
    result[1] = u8::from(prefix);
    result[2..22].copy_from_slice(address.as_bytes());
    result
}

pub enum StorageKeyKind {
    Normal([u8; 54]),
    Generation([u8; 58]),
}

impl AsRef<[u8]> for StorageKeyKind {
    fn as_ref(&self) -> &[u8] {
        match &self {
            Self::Normal(v) => v,
            Self::Generation(v) => v,
        }
    }
}

#[must_use]
pub fn storage_to_key(address: &Address, key: &H256, generation: u32) -> StorageKeyKind {
    if generation == 0 {
        StorageKeyKind::Normal(normal_storage_key(address, key))
    } else {
        StorageKeyKind::Generation(generation_storage_key(address, key, generation))
    }
}

#[allow(dead_code)]
fn normal_storage_key(address: &Address, key: &H256) -> [u8; 54] {
    let mut result = [0u8; 54];
    result[0] = u8::from(VersionPrefix::V1);
    result[1] = u8::from(KeyPrefix::Storage);
    result[2..22].copy_from_slice(address.as_bytes());
    result[22..54].copy_from_slice(&key.0);
    result
}

#[allow(dead_code)]
fn generation_storage_key(address: &Address, key: &H256, generation: u32) -> [u8; 58] {
    let mut result = [0u8; 58];
    result[0] = u8::from(VersionPrefix::V1);
    result[1] = u8::from(KeyPrefix::Storage);
    result[2..22].copy_from_slice(address.as_bytes());
    result[22..26].copy_from_slice(&generation.to_le_bytes());
    result[26..58].copy_from_slice(&key.0);
    result
}

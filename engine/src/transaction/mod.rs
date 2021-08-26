use prelude::{Address, TryFrom, Vec, U256};
use rlp::{Decodable, DecoderError, Rlp};

pub(crate) mod access_list;
pub(crate) mod legacy;

use access_list::AccessTuple;
pub use legacy::{LegacyEthSignedTransaction, LegacyEthTransaction};

/// Typed Transaction Envelope (see https://eips.ethereum.org/EIPS/eip-2718)
#[derive(Eq, PartialEq)]
pub enum EthTransaction {
    Legacy(LegacyEthSignedTransaction),
    AccessList(access_list::AccessListEthSignedTransaction),
}

impl EthTransaction {
    pub fn chain_id(&self) -> Option<u64> {
        match self {
            Self::Legacy(tx) => tx.chain_id(),
            Self::AccessList(tx) => Some(tx.transaction_data.chain_id),
        }
    }

    pub fn sender(&self) -> Option<Address> {
        match self {
            Self::Legacy(tx) => tx.sender(),
            Self::AccessList(tx) => tx.sender(),
        }
    }

    pub fn nonce(&self) -> &U256 {
        match self {
            Self::Legacy(tx) => &tx.transaction.nonce,
            Self::AccessList(tx) => &tx.transaction_data.nonce,
        }
    }

    pub fn intrinsic_gas(&self, config: &evm::Config) -> Option<u64> {
        match self {
            Self::Legacy(tx) => tx.transaction.intrinsic_gas(config),
            Self::AccessList(tx) => tx.transaction_data.intrinsic_gas(config),
        }
    }

    pub fn gas_limit(&self) -> U256 {
        match self {
            Self::Legacy(tx) => tx.transaction.gas,
            Self::AccessList(tx) => tx.transaction_data.gas_limit,
        }
    }

    pub fn gas_price(&self) -> U256 {
        match self {
            Self::Legacy(tx) => tx.transaction.gas_price,
            Self::AccessList(tx) => tx.transaction_data.gas_price,
        }
    }

    pub fn destructure(
        self,
    ) -> (
        prelude::types::Wei,
        Option<u64>,
        Vec<u8>,
        Option<Address>,
        Vec<AccessTuple>,
    ) {
        use prelude::TryInto;
        match self {
            Self::Legacy(tx) => {
                let tx = tx.transaction;
                (tx.value, tx.gas.try_into().ok(), tx.data, tx.to, Vec::new())
            }
            Self::AccessList(tx) => {
                let tx = tx.transaction_data;
                (
                    tx.value,
                    tx.gas_limit.try_into().ok(),
                    tx.data,
                    tx.to,
                    tx.access_list,
                )
            }
        }
    }
}

impl TryFrom<&[u8]> for EthTransaction {
    type Error = ParseTransactionError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes[0] == access_list::TYPE_BYTE {
            let access_list_tx =
                access_list::AccessListEthSignedTransaction::decode(&Rlp::new(&bytes[1..]))?;
            Ok(Self::AccessList(access_list_tx))
        } else if bytes[0] <= 0x7f {
            Err(ParseTransactionError::UnknownTransactionType)
        } else if bytes[0] == 0xff {
            Err(ParseTransactionError::ReservedSentinel)
        } else {
            let legacy = LegacyEthSignedTransaction::decode(&Rlp::new(bytes))?;
            Ok(Self::Legacy(legacy))
        }
    }
}

pub enum ParseTransactionError {
    UnknownTransactionType,
    // Per the EIP-2718 spec 0xff is a reserved value
    ReservedSentinel,
    RlpDecodeError(DecoderError),
}

impl From<DecoderError> for ParseTransactionError {
    fn from(e: DecoderError) -> Self {
        Self::RlpDecodeError(e)
    }
}

impl AsRef<[u8]> for ParseTransactionError {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::UnknownTransactionType => b"ERR_UNKNOWN_TX_TYPE",
            Self::ReservedSentinel => b"ERR_RESERVED_LEADING_TX_BYTE",
            Self::RlpDecodeError(_) => b"ERR_TX_RLP_DECODE",
        }
    }
}

fn rlp_extract_to(rlp: &Rlp<'_>, index: usize) -> Result<Option<Address>, DecoderError> {
    let value = rlp.at(index)?;
    if value.is_empty() {
        if value.is_data() {
            Ok(None)
        } else {
            Err(rlp::DecoderError::RlpExpectedToBeData)
        }
    } else {
        let v: Address = value.as_val()?;
        if v == Address::zero() {
            Ok(None)
        } else {
            Ok(Some(v))
        }
    }
}

fn intrinsic_gas(
    is_contract_creation: bool,
    data: &[u8],
    access_list: &[access_list::AccessTuple],
    config: &evm::Config,
) -> Option<u64> {
    let base_gas = if is_contract_creation {
        config.gas_transaction_create
    } else {
        config.gas_transaction_call
    };

    let num_zero_bytes = data.iter().filter(|b| **b == 0).count();
    let num_non_zero_bytes = data.len() - num_zero_bytes;

    let gas_zero_bytes = config
        .gas_transaction_zero_data
        .checked_mul(num_zero_bytes as u64)?;
    let gas_non_zero_bytes = config
        .gas_transaction_non_zero_data
        .checked_mul(num_non_zero_bytes as u64)?;

    let gas_access_list_address = config
        .gas_access_list_address
        .checked_mul(access_list.len() as u64)?;
    let gas_access_list_storage = config.gas_access_list_storage_key.checked_mul(
        access_list
            .iter()
            .map(|a| a.storage_keys.len() as u64)
            .sum(),
    )?;

    base_gas
        .checked_add(gas_zero_bytes)
        .and_then(|gas| gas.checked_add(gas_non_zero_bytes))
        .and_then(|gas| gas.checked_add(gas_access_list_address))
        .and_then(|gas| gas.checked_add(gas_access_list_storage))
}

fn vrs_to_arr(v: u8, r: U256, s: U256) -> [u8; 65] {
    let mut result = [0u8; 65]; // (r, s, v), typed (uint256, uint256, uint8)
    r.to_big_endian(&mut result[0..32]);
    s.to_big_endian(&mut result[32..64]);
    result[64] = v;
    result
}

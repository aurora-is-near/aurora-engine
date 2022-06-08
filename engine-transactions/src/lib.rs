#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(alloc_error_handler))]

use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{vec, Vec, H160, U256};
use eip_2930::AccessTuple;
use rlp::{Decodable, DecoderError, Rlp};

pub mod backwards_compatibility;
pub mod eip_1559;
pub mod eip_2930;
pub mod legacy;

/// Typed Transaction Envelope (see https://eips.ethereum.org/EIPS/eip-2718)
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum EthTransactionKind {
    Legacy(legacy::LegacyEthSignedTransaction),
    Eip2930(eip_2930::SignedTransaction2930),
    Eip1559(eip_1559::SignedTransaction1559),
}

impl TryFrom<&[u8]> for EthTransactionKind {
    type Error = ParseTransactionError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes[0] == eip_2930::TYPE_BYTE {
            Ok(Self::Eip2930(eip_2930::SignedTransaction2930::decode(
                &Rlp::new(&bytes[1..]),
            )?))
        } else if bytes[0] == eip_1559::TYPE_BYTE {
            Ok(Self::Eip1559(eip_1559::SignedTransaction1559::decode(
                &Rlp::new(&bytes[1..]),
            )?))
        } else if bytes[0] <= 0x7f {
            Err(ParseTransactionError::UnknownTransactionType)
        } else if bytes[0] == 0xff {
            Err(ParseTransactionError::ReservedSentinel)
        } else {
            let legacy = legacy::LegacyEthSignedTransaction::decode(&Rlp::new(bytes))?;
            Ok(Self::Legacy(legacy))
        }
    }
}

impl<'a> From<&'a EthTransactionKind> for Vec<u8> {
    fn from(tx: &'a EthTransactionKind) -> Self {
        let mut stream = rlp::RlpStream::new();
        match &tx {
            EthTransactionKind::Legacy(tx) => {
                stream.append(tx);
            }
            EthTransactionKind::Eip1559(tx) => {
                stream.append(&eip_1559::TYPE_BYTE);
                stream.append(tx);
            }
            EthTransactionKind::Eip2930(tx) => {
                stream.append(&eip_2930::TYPE_BYTE);
                stream.append(tx);
            }
        }
        stream.out().to_vec()
    }
}

/// A normalized Ethereum transaction which can be created from older
/// transactions.
pub struct NormalizedEthTransaction {
    pub address: Option<Address>,
    pub chain_id: Option<u64>,
    pub nonce: U256,
    pub gas_limit: U256,
    pub max_priority_fee_per_gas: U256,
    pub max_fee_per_gas: U256,
    pub to: Option<Address>,
    pub value: Wei,
    pub data: Vec<u8>,
    pub access_list: Vec<AccessTuple>,
}

impl From<EthTransactionKind> for NormalizedEthTransaction {
    fn from(kind: EthTransactionKind) -> Self {
        use EthTransactionKind::*;
        match kind {
            Legacy(tx) => Self {
                address: tx.sender(),
                chain_id: tx.chain_id(),
                nonce: tx.transaction.nonce,
                gas_limit: tx.transaction.gas_limit,
                max_priority_fee_per_gas: tx.transaction.gas_price,
                max_fee_per_gas: tx.transaction.gas_price,
                to: tx.transaction.to,
                value: tx.transaction.value,
                data: tx.transaction.data,
                access_list: vec![],
            },
            Eip2930(tx) => Self {
                address: tx.sender(),
                chain_id: Some(tx.transaction.chain_id),
                nonce: tx.transaction.nonce,
                gas_limit: tx.transaction.gas_limit,
                max_priority_fee_per_gas: tx.transaction.gas_price,
                max_fee_per_gas: tx.transaction.gas_price,
                to: tx.transaction.to,
                value: tx.transaction.value,
                data: tx.transaction.data,
                access_list: tx.transaction.access_list,
            },
            Eip1559(tx) => Self {
                address: tx.sender(),
                chain_id: Some(tx.transaction.chain_id),
                nonce: tx.transaction.nonce,
                gas_limit: tx.transaction.gas_limit,
                max_priority_fee_per_gas: tx.transaction.max_priority_fee_per_gas,
                max_fee_per_gas: tx.transaction.max_fee_per_gas,
                to: tx.transaction.to,
                value: tx.transaction.value,
                data: tx.transaction.data,
                access_list: tx.transaction.access_list,
            },
        }
    }
}

impl NormalizedEthTransaction {
    pub fn intrinsic_gas(&self, config: &evm::Config) -> Option<u64> {
        let is_contract_creation = self.to.is_none();

        let base_gas = if is_contract_creation {
            config.gas_transaction_create
        } else {
            config.gas_transaction_call
        };

        let num_zero_bytes = self.data.iter().filter(|b| **b == 0).count();
        let num_non_zero_bytes = self.data.len() - num_zero_bytes;

        let gas_zero_bytes = config
            .gas_transaction_zero_data
            .checked_mul(num_zero_bytes as u64)?;
        let gas_non_zero_bytes = config
            .gas_transaction_non_zero_data
            .checked_mul(num_non_zero_bytes as u64)?;

        let gas_access_list_address = config
            .gas_access_list_address
            .checked_mul(self.access_list.len() as u64)?;
        let gas_access_list_storage = config.gas_access_list_storage_key.checked_mul(
            self.access_list
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
}

#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum ParseTransactionError {
    UnknownTransactionType,
    // Per the EIP-2718 spec 0xff is a reserved value
    ReservedSentinel,
    #[cfg_attr(feature = "serde", serde(serialize_with = "decoder_err_to_str"))]
    RlpDecodeError(DecoderError),
}

#[cfg(feature = "serde")]
fn decoder_err_to_str<S: serde::Serializer>(err: &DecoderError, ser: S) -> Result<S::Ok, S::Error> {
    ser.serialize_str(&format!("{:?}", err))
}

impl ParseTransactionError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UnknownTransactionType => "ERR_UNKNOWN_TX_TYPE",
            Self::ReservedSentinel => "ERR_RESERVED_LEADING_TX_BYTE",
            Self::RlpDecodeError(_) => "ERR_TX_RLP_DECODE",
        }
    }
}

impl From<DecoderError> for ParseTransactionError {
    fn from(e: DecoderError) -> Self {
        Self::RlpDecodeError(e)
    }
}

impl AsRef<[u8]> for ParseTransactionError {
    fn as_ref(&self) -> &[u8] {
        self.as_str().as_bytes()
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
        let v: H160 = value.as_val()?;
        let addr = Address::new(v);
        Ok(Some(addr))
    }
}

fn vrs_to_arr(v: u8, r: U256, s: U256) -> [u8; 65] {
    let mut result = [0u8; 65]; // (r, s, v), typed (uint256, uint256, uint8)
    r.to_big_endian(&mut result[0..32]);
    s.to_big_endian(&mut result[32..64]);
    result[64] = v;
    result
}

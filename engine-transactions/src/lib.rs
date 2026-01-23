#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{H160, U256, Vec, vec};
use aurora_evm::executor::stack::Authorization;
use eip_2930::AccessTuple;
use rlp::{Decodable, DecoderError, Rlp};

pub mod backwards_compatibility;
pub mod eip_1559;
pub mod eip_2930;
pub mod eip_4844;
pub mod eip_7702;
pub mod legacy;

const INITCODE_WORD_COST: u64 = 2;

/// Typed Transaction Envelope (see `https://eips.ethereum.org/EIPS/eip-2718`)
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum EthTransactionKind {
    Legacy(legacy::LegacyEthSignedTransaction),
    Eip2930(eip_2930::SignedTransaction2930),
    Eip1559(eip_1559::SignedTransaction1559),
    Eip7702(eip_7702::SignedTransaction7702),
}

impl TryFrom<&[u8]> for EthTransactionKind {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.is_empty() {
            Err(Error::EmptyInput)
        } else if bytes[0] == eip_2930::TYPE_BYTE {
            Ok(Self::Eip2930(eip_2930::SignedTransaction2930::decode(
                &Rlp::new(&bytes[1..]),
            )?))
        } else if bytes[0] == eip_1559::TYPE_BYTE {
            Ok(Self::Eip1559(eip_1559::SignedTransaction1559::decode(
                &Rlp::new(&bytes[1..]),
            )?))
        } else if bytes[0] == eip_4844::TYPE_BYTE {
            Err(Error::UnsupportedTransactionEip4844)
        } else if bytes[0] == eip_7702::TYPE_BYTE {
            Ok(Self::Eip7702(eip_7702::SignedTransaction7702::decode(
                &Rlp::new(&bytes[1..]),
            )?))
        } else if bytes[0] <= 0x7f {
            Err(Error::UnknownTransactionType)
        } else if bytes[0] == 0xff {
            Err(Error::ReservedSentinel)
        } else {
            let legacy = legacy::LegacyEthSignedTransaction::decode(&Rlp::new(bytes))?;
            Ok(Self::Legacy(legacy))
        }
    }
}

impl From<&EthTransactionKind> for Vec<u8> {
    fn from(tx: &EthTransactionKind) -> Self {
        let mut stream = rlp::RlpStream::new();
        match &tx {
            EthTransactionKind::Legacy(tx) => {
                stream.append(tx);
            }
            EthTransactionKind::Eip2930(tx) => {
                stream.append(&eip_2930::TYPE_BYTE);
                stream.append(tx);
            }
            EthTransactionKind::Eip1559(tx) => {
                stream.append(&eip_1559::TYPE_BYTE);
                stream.append(tx);
            }
            EthTransactionKind::Eip7702(tx) => {
                stream.append(&eip_7702::TYPE_BYTE);
                stream.append(tx);
            }
        }
        stream.out().to_vec()
    }
}

/// A normalized Ethereum transaction which can be created from older
/// transactions.
pub struct NormalizedEthTransaction {
    pub address: Address,
    pub chain_id: Option<u64>,
    pub nonce: U256,
    pub gas_limit: U256,
    pub max_priority_fee_per_gas: U256,
    pub max_fee_per_gas: U256,
    pub to: Option<Address>,
    pub value: Wei,
    pub data: Vec<u8>,
    pub access_list: Vec<AccessTuple>,
    // Contains additional information - `chain_id` for each authorization item
    pub authorization_list: Vec<Authorization>,
}

impl TryFrom<EthTransactionKind> for NormalizedEthTransaction {
    type Error = Error;

    fn try_from(kind: EthTransactionKind) -> Result<Self, Self::Error> {
        use EthTransactionKind::{Eip1559, Eip2930, Eip7702, Legacy};
        Ok(match kind {
            Legacy(tx) => Self {
                address: tx.sender()?,
                chain_id: tx.chain_id(),
                nonce: tx.transaction.nonce,
                gas_limit: tx.transaction.gas_limit,
                max_priority_fee_per_gas: tx.transaction.gas_price,
                max_fee_per_gas: tx.transaction.gas_price,
                to: tx.transaction.to,
                value: tx.transaction.value,
                data: tx.transaction.data,
                access_list: vec![],
                authorization_list: vec![],
            },
            Eip2930(tx) => Self {
                address: tx.sender()?,
                chain_id: Some(tx.transaction.chain_id),
                nonce: tx.transaction.nonce,
                gas_limit: tx.transaction.gas_limit,
                max_priority_fee_per_gas: tx.transaction.gas_price,
                max_fee_per_gas: tx.transaction.gas_price,
                to: tx.transaction.to,
                value: tx.transaction.value,
                data: tx.transaction.data,
                access_list: tx.transaction.access_list,
                authorization_list: vec![],
            },
            Eip1559(tx) => Self {
                address: tx.sender()?,
                chain_id: Some(tx.transaction.chain_id),
                nonce: tx.transaction.nonce,
                gas_limit: tx.transaction.gas_limit,
                max_priority_fee_per_gas: tx.transaction.max_priority_fee_per_gas,
                max_fee_per_gas: tx.transaction.max_fee_per_gas,
                to: tx.transaction.to,
                value: tx.transaction.value,
                data: tx.transaction.data,
                access_list: tx.transaction.access_list,
                authorization_list: vec![],
            },
            Eip7702(tx) => Self {
                address: tx.sender()?,
                chain_id: Some(tx.transaction.chain_id),
                nonce: tx.transaction.nonce,
                gas_limit: tx.transaction.gas_limit,
                max_priority_fee_per_gas: tx.transaction.max_priority_fee_per_gas,
                max_fee_per_gas: tx.transaction.max_fee_per_gas,
                to: Some(tx.transaction.to),
                value: tx.transaction.value,
                data: tx.transaction.data.clone(),
                access_list: tx.transaction.access_list.clone(),
                authorization_list: tx.authorization_list()?,
            },
        })
    }
}

impl NormalizedEthTransaction {
    #[allow(clippy::naive_bytecount)]
    pub fn intrinsic_gas(&self, config: &aurora_evm::Config) -> Result<u64, Error> {
        let is_contract_creation = self.to.is_none();

        let base_gas = if is_contract_creation {
            config.gas_transaction_create + init_code_cost(config, &self.data)?
        } else {
            config.gas_transaction_call
        };

        let num_zero_bytes = u64::try_from(self.data.iter().filter(|b| **b == 0).count())
            .map_err(|_e| Error::IntegerConversion)?;
        let gas_zero_bytes = config
            .gas_transaction_zero_data
            .checked_mul(num_zero_bytes)
            .ok_or(Error::GasOverflow)?;

        let data_len = u64::try_from(self.data.len()).map_err(|_e| Error::IntegerConversion)?;
        let num_non_zero_bytes = data_len - num_zero_bytes;
        let gas_non_zero_bytes = config
            .gas_transaction_non_zero_data
            .checked_mul(num_non_zero_bytes)
            .ok_or(Error::GasOverflow)?;

        let access_list_len =
            u64::try_from(self.access_list.len()).map_err(|_e| Error::IntegerConversion)?;
        let gas_access_list_address = config
            .gas_access_list_address
            .checked_mul(access_list_len)
            .ok_or(Error::GasOverflow)?;

        let gas_access_list_storage = config
            .gas_access_list_storage_key
            .checked_mul(
                u64::try_from(
                    self.access_list
                        .iter()
                        .map(|a| a.storage_keys.len())
                        .sum::<usize>(),
                )
                .map_err(|_e| Error::IntegerConversion)?,
            )
            .ok_or(Error::GasOverflow)?;

        let gas_authorization_list = if config.has_authorization_list {
            config
                .gas_per_auth_base_cost
                .checked_mul(
                    u64::try_from(self.authorization_list.len())
                        .map_err(|_e| Error::IntegerConversion)?,
                )
                .ok_or(Error::GasOverflow)?
        } else {
            0
        };

        base_gas
            .checked_add(gas_zero_bytes)
            .and_then(|gas| gas.checked_add(gas_non_zero_bytes))
            .and_then(|gas| gas.checked_add(gas_access_list_address))
            .and_then(|gas| gas.checked_add(gas_access_list_storage))
            .and_then(|gas| gas.checked_add(gas_authorization_list))
            .ok_or(Error::GasOverflow)
    }

    #[allow(clippy::naive_bytecount)]
    pub fn floor_gas(&self, config: &aurora_evm::Config) -> Result<u64, Error> {
        if config.has_floor_gas {
            let num_zero_bytes = u64::try_from(self.data.iter().filter(|b| **b == 0).count())
                .map_err(|_e| Error::IntegerConversion)?;
            let data_len = u64::try_from(self.data.len()).map_err(|_e| Error::IntegerConversion)?;
            let num_non_zero_bytes = data_len
                .checked_sub(num_zero_bytes)
                .ok_or(Error::GasOverflow)?;

            let base_gas = config.gas_transaction_call;
            let tokens_in_calldata = num_non_zero_bytes
                .checked_mul(4)
                .and_then(|gas| gas.checked_add(num_zero_bytes))
                .ok_or(Error::GasOverflow)?;

            tokens_in_calldata
                .checked_mul(config.total_cost_floor_per_token)
                .and_then(|gas| gas.checked_add(base_gas))
                .ok_or(Error::GasOverflow)
        } else {
            Ok(0)
        }
    }
}

fn init_code_cost(config: &aurora_evm::Config, data: &[u8]) -> Result<u64, Error> {
    // As per EIP-3860:
    // We define initcode_cost(initcode) to equal INITCODE_WORD_COST * ceil(len(initcode) / 32).
    let init_code_cost = if config.max_initcode_size.is_some() {
        let data_len = u64::try_from(data.len()).map_err(|_| Error::IntegerConversion)?;
        data_len.div_ceil(32) * INITCODE_WORD_COST
    } else {
        0
    };

    Ok(init_code_cost)
}

#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Error {
    UnknownTransactionType,
    EmptyInput,
    // Per the EIP-2718 spec 0xff is a reserved value
    ReservedSentinel,
    InvalidV,
    EcRecover,
    GasOverflow,
    IntegerConversion,
    #[cfg_attr(feature = "serde", serde(serialize_with = "decoder_err_to_str"))]
    RlpDecodeError(DecoderError),
    UnsupportedTransactionEip4844,
    EmptyAuthorizationList,
}

#[cfg(feature = "serde")]
fn decoder_err_to_str<S: serde::Serializer>(err: &DecoderError, ser: S) -> Result<S::Ok, S::Error> {
    ser.serialize_str(&format!("{err:?}"))
}

impl Error {
    #[must_use]
    pub const fn as_str(&self) -> &str {
        match self {
            Self::UnknownTransactionType => "ERR_UNKNOWN_TX_TYPE",
            Self::EmptyInput => "ERR_EMPTY_TX_INPUT",
            Self::ReservedSentinel => "ERR_RESERVED_LEADING_TX_BYTE",
            Self::InvalidV => "ERR_INVALID_V",
            Self::EcRecover => "ERR_ECRECOVER",
            Self::GasOverflow => "ERR_GAS_OVERFLOW",
            Self::IntegerConversion => "ERR_INTEGER_CONVERSION",
            Self::RlpDecodeError(_) => "ERR_TX_RLP_DECODE",
            Self::UnsupportedTransactionEip4844 => "ERR_UNSUPPORTED_TX_EIP4844",
            Self::EmptyAuthorizationList => "ERR_EMPTY_AUTHORIZATION_LIST",
        }
    }
}

impl From<DecoderError> for Error {
    fn from(e: DecoderError) -> Self {
        Self::RlpDecodeError(e)
    }
}

impl AsRef<[u8]> for Error {
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
            Err(DecoderError::RlpExpectedToBeData)
        }
    } else {
        let v: H160 = value.as_val()?;
        let addr = Address::new(v);
        Ok(Some(addr))
    }
}

fn vrs_to_arr(v: u8, r: U256, s: U256) -> [u8; 65] {
    let mut result = [0u8; 65]; // (r, s, v), typed (uint256, uint256, uint8)
    result[..32].copy_from_slice(&r.to_big_endian());
    result[32..64].copy_from_slice(&s.to_big_endian());
    result[64] = v;
    result
}

#[cfg(test)]
mod tests {
    use super::{Error, EthTransactionKind, INITCODE_WORD_COST};
    use crate::{eip_1559, eip_2930, eip_7702};
    use aurora_engine_types::types::{Address, Wei};
    use aurora_engine_types::{H160, H256, U256};
    use aurora_evm::executor::stack::Authorization;

    #[test]
    fn test_try_parse_empty_input() {
        assert!(matches!(
            EthTransactionKind::try_from([].as_ref()),
            Err(Error::EmptyInput)
        ));

        // If the first byte is present, then empty bytes will be passed in to
        // the RLP parsing. Let's also check this is not a problem.
        assert!(matches!(
            EthTransactionKind::try_from([eip_1559::TYPE_BYTE].as_ref()),
            Err(Error::RlpDecodeError(_))
        ));
        assert!(matches!(
            EthTransactionKind::try_from([eip_2930::TYPE_BYTE].as_ref()),
            Err(Error::RlpDecodeError(_))
        ));
        assert!(matches!(
            EthTransactionKind::try_from([0x80].as_ref()),
            Err(Error::RlpDecodeError(_))
        ));
        assert!(matches!(
            EthTransactionKind::try_from([eip_7702::TYPE_BYTE].as_ref()),
            Err(Error::RlpDecodeError(_))
        ));
    }

    #[test]
    fn test_initcode_cost() {
        let config = aurora_evm::Config::cancun();

        let data = [0u8; 60];
        let cost = super::init_code_cost(&config, &data).unwrap();
        assert_eq!(cost, 4);

        let data = [0u8; 30];
        let cost = super::init_code_cost(&config, &data).unwrap();
        assert_eq!(cost, 2);

        let data = [0u8; 129];
        let cost = super::init_code_cost(&config, &data).unwrap();
        assert_eq!(cost, 10);

        let data = [0u8; 1000];
        let cost = super::init_code_cost(&config, &data).unwrap();
        assert_eq!(cost, 64);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn test_intrinsic_gas() {
        use super::NormalizedEthTransaction;

        let config = aurora_evm::Config::prague();

        // Test a simple transaction with no data
        let tx = NormalizedEthTransaction {
            address: Address::default(),
            chain_id: Some(1),
            nonce: U256::zero(),
            gas_limit: U256::from(21000),
            max_priority_fee_per_gas: U256::from(1000000000u64),
            max_fee_per_gas: U256::from(1000000000u64),
            to: Some(Address::default()),
            value: Wei::zero(),
            data: vec![],
            access_list: vec![],
            authorization_list: vec![],
        };
        let gas = tx.intrinsic_gas(&config).unwrap();

        assert_eq!(gas, config.gas_transaction_call);

        // Test transaction with zero bytes
        let tx = NormalizedEthTransaction {
            address: Address::default(),
            chain_id: Some(1),
            nonce: U256::zero(),
            gas_limit: U256::from(21000),
            max_priority_fee_per_gas: U256::from(1000000000u64),
            max_fee_per_gas: U256::from(1000000000u64),
            to: Some(Address::default()),
            value: Wei::zero(),
            data: vec![0u8; 10],
            access_list: vec![],
            authorization_list: vec![],
        };
        let gas = tx.intrinsic_gas(&config).unwrap();

        assert_eq!(
            gas,
            config.gas_transaction_call + config.gas_transaction_zero_data * 10
        );

        // Test transaction with non-zero bytes
        let tx = NormalizedEthTransaction {
            address: Address::default(),
            chain_id: Some(1),
            nonce: U256::zero(),
            gas_limit: U256::from(21000),
            max_priority_fee_per_gas: U256::from(1000000000u64),
            max_fee_per_gas: U256::from(1000000000u64),
            to: Some(Address::default()),
            value: Wei::zero(),
            data: vec![1u8; 10],
            access_list: vec![],
            authorization_list: vec![],
        };
        let gas = tx.intrinsic_gas(&config).unwrap();

        assert_eq!(
            gas,
            config.gas_transaction_call + config.gas_transaction_non_zero_data * 10
        );

        // Test transaction with mixed zero and non-zero bytes
        let tx = NormalizedEthTransaction {
            address: Address::default(),
            chain_id: Some(1),
            nonce: U256::zero(),
            gas_limit: U256::from(21000),
            max_priority_fee_per_gas: U256::from(1000000000u64),
            max_fee_per_gas: U256::from(1000000000u64),
            to: Some(Address::default()),
            value: Wei::zero(),
            data: vec![0, 1, 0, 1, 0],
            access_list: vec![],
            authorization_list: vec![],
        };
        let gas = tx.intrinsic_gas(&config).unwrap();
        let expected = config.gas_transaction_call
            + config.gas_transaction_zero_data * 3
            + config.gas_transaction_non_zero_data * 2;

        assert_eq!(gas, expected);

        // Test contract creation
        let tx = NormalizedEthTransaction {
            address: Address::default(),
            chain_id: Some(1),
            nonce: U256::zero(),
            gas_limit: U256::from(21000),
            max_priority_fee_per_gas: U256::from(1000000000u64),
            max_fee_per_gas: U256::from(1000000000u64),
            to: None,
            value: Wei::zero(),
            data: vec![1u8; 32],
            access_list: vec![],
            authorization_list: vec![],
        };
        let gas = tx.intrinsic_gas(&config).unwrap();
        let expected = config.gas_transaction_create
            + INITCODE_WORD_COST
            + config.gas_transaction_non_zero_data * 32;
        assert_eq!(gas, expected);

        // Test transaction with an access list
        let access_tuple = eip_2930::AccessTuple {
            address: Address::default().raw(),
            storage_keys: vec![H256::zero(), H256::zero()],
        };
        let tx = NormalizedEthTransaction {
            address: Address::default(),
            chain_id: Some(1),
            nonce: U256::zero(),
            gas_limit: U256::from(21000),
            max_priority_fee_per_gas: U256::from(1000000000u64),
            max_fee_per_gas: U256::from(1000000000u64),
            to: Some(Address::default()),
            value: Wei::zero(),
            data: vec![],
            access_list: vec![access_tuple],
            authorization_list: vec![],
        };
        let gas = tx.intrinsic_gas(&config).unwrap();
        let expected = config.gas_transaction_call
            + config.gas_access_list_address
            + config.gas_access_list_storage_key * 2;

        assert_eq!(gas, expected);

        // Test transaction with an authorization list
        let authorization = Authorization {
            authority: H160::default(),
            address: Address::default().raw(),
            nonce: 0,
            is_valid: false,
        };
        let tx = NormalizedEthTransaction {
            address: Address::default(),
            chain_id: Some(1),
            nonce: U256::zero(),
            gas_limit: U256::from(21000),
            max_priority_fee_per_gas: U256::from(1000000000u64),
            max_fee_per_gas: U256::from(1000000000u64),
            to: Some(Address::default()),
            value: Wei::zero(),
            data: vec![],
            access_list: vec![],
            authorization_list: vec![authorization],
        };
        let gas = tx.intrinsic_gas(&config).unwrap();
        let expected = config.gas_transaction_call + config.gas_per_auth_base_cost;
        assert_eq!(gas, expected);
    }

    fn create_test_transaction(data: Vec<u8>) -> super::NormalizedEthTransaction {
        super::NormalizedEthTransaction {
            address: Address::default(),
            chain_id: Some(1),
            nonce: U256::zero(),
            gas_limit: U256::from(21000),
            max_priority_fee_per_gas: U256::from(1000000000u64),
            max_fee_per_gas: U256::from(1000000000u64),
            to: Some(Address::default()),
            value: Wei::zero(),
            data,
            access_list: vec![],
            authorization_list: vec![],
        }
    }

    #[test]
    fn test_floor_gas_disabled() {
        let config = aurora_evm::Config::cancun();
        let tx = create_test_transaction(vec![1u8; 10]);
        let gas = tx.floor_gas(&config).unwrap();

        assert_eq!(gas, 0);
    }

    #[test]
    fn test_floor_gas_empty_data() {
        let config = aurora_evm::Config::prague();
        let tx = create_test_transaction(vec![]);
        let gas = tx.floor_gas(&config).unwrap();

        assert_eq!(gas, 21000);
    }

    #[test]
    fn test_floor_gas_all_zero_bytes() {
        let config = aurora_evm::Config::prague();
        let tx = create_test_transaction(vec![0u8; 10]);
        let gas = tx.floor_gas(&config).unwrap();

        // tokens_in_calldata = 0 * 4 + 10 = 10
        // floor_gas = 10 * 10 + 21000 = 21100
        assert_eq!(gas, 21100);
    }

    #[test]
    fn test_floor_gas_all_non_zero_bytes() {
        let config = aurora_evm::Config::prague();
        let tx = create_test_transaction(vec![1u8; 10]);
        let gas = tx.floor_gas(&config).unwrap();

        // tokens_in_calldata = 10 * 4 + 0 = 40
        // floor_gas = 40 * 10 + 21000 = 21400
        assert_eq!(gas, 21400);
    }

    #[test]
    fn test_floor_gas_mixed_bytes() {
        let config = aurora_evm::Config::prague();
        let tx = create_test_transaction(vec![0, 1, 0, 1, 0, 1, 1, 1]);
        let gas = tx.floor_gas(&config).unwrap();

        // num_zero_bytes = 3
        // num_non_zero_bytes = 5
        // tokens_in_calldata = 5 * 4 + 3 = 23
        // floor_gas = 23 * 10 + 21000 = 21230
        assert_eq!(gas, 21230);
    }

    #[test]
    fn test_floor_gas_large_data() {
        let config = aurora_evm::Config::prague();
        let tx = create_test_transaction(vec![1u8; 1000]);
        let gas = tx.floor_gas(&config).unwrap();

        // tokens_in_calldata = 1000 * 4 + 0 = 4000
        // floor_gas = 4000 * 10 + 21000 = 61000
        assert_eq!(gas, 61000);
    }

    #[test]
    fn test_floor_gas_overflow_on_mul_cost_per_token() {
        let mut config = aurora_evm::Config::prague();
        config.total_cost_floor_per_token = u64::MAX;

        let tx = create_test_transaction(vec![1u8; 10]);
        let result = tx.floor_gas(&config);

        assert!(matches!(result, Err(Error::GasOverflow)));
    }

    #[test]
    fn test_floor_gas_overflow_on_add_base() {
        let mut config = aurora_evm::Config::prague();
        config.has_floor_gas = true;
        config.total_cost_floor_per_token = u64::MAX;

        let tx = create_test_transaction(vec![0u8; 1]);
        let result = tx.floor_gas(&config);

        assert!(matches!(result, Err(Error::GasOverflow)));
    }

    #[test]
    fn test_floor_gas_with_different_cost_per_token() {
        let mut config = aurora_evm::Config::prague();
        config.has_floor_gas = true;
        config.total_cost_floor_per_token = 500;

        let tx = create_test_transaction(vec![1u8; 5]);
        let gas = tx.floor_gas(&config).unwrap();

        // tokens_in_calldata = 5 * 4 + 0 = 20
        // floor_gas = 20 * 500 + 21000 = 31000
        assert_eq!(gas, 31000);
    }
}

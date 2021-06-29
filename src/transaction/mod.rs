use crate::prelude::{Address, TryFrom};
use rlp::{Decodable, DecoderError, Rlp};

pub(crate) mod access_list;
pub(crate) mod legacy;

pub use legacy::{LegacyEthSignedTransaction, LegacyEthTransaction};

/// Typed Transaction Envelope (see https://eips.ethereum.org/EIPS/eip-2718)
#[derive(Eq, PartialEq)]
pub enum EthTransaction {
    Legacy(LegacyEthSignedTransaction),
    AccessList(access_list::AccessListEthSignedTransaction),
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

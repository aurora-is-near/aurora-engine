use crate::Error;
use aurora_engine_precompiles::secp256k1::ecrecover;
use aurora_engine_sdk as sdk;
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{Vec, U256};
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct TransactionLegacy {
    /// A monotonically increasing transaction counter for this sender
    pub nonce: U256,
    /// The fee the sender pays per unit of gas
    pub gas_price: U256,
    /// The maximum amount of gas units consumed by the transaction
    pub gas_limit: U256,
    /// The receiving address (`None` for the zero address)
    pub to: Option<Address>,
    /// The amount of ETH to transfer
    pub value: Wei,
    /// Arbitrary binary data for a contract call invocation
    pub data: Vec<u8>,
}

impl TransactionLegacy {
    pub fn rlp_append_unsigned(&self, s: &mut RlpStream, chain_id: Option<u64>) {
        s.begin_list(if chain_id.is_none() { 6 } else { 9 });
        s.append(&self.nonce);
        s.append(&self.gas_price);
        s.append(&self.gas_limit);
        match self.to.as_ref() {
            None => s.append(&""),
            Some(address) => s.append(&address.raw()),
        };
        s.append(&self.value.raw());
        s.append(&self.data);
        if let Some(chain_id) = chain_id {
            s.append(&chain_id);
            s.append(&0u8);
            s.append(&0u8);
        }
    }

    /// Returns self.gas as a u64, or None if self.gas > u64::MAX
    #[allow(unused)]
    pub fn get_gas_limit(&self) -> Option<u64> {
        self.gas_limit.try_into().ok()
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct LegacyEthSignedTransaction {
    /// The unsigned transaction data
    pub transaction: TransactionLegacy,
    /// The ECDSA recovery ID
    pub v: u64,
    /// The first ECDSA signature output
    pub r: U256,
    /// The second ECDSA signature output
    pub s: U256,
}

impl LegacyEthSignedTransaction {
    /// Returns sender of given signed transaction by doing ecrecover on the signature.
    pub fn sender(&self) -> Result<Address, Error> {
        let mut rlp_stream = RlpStream::new();
        // See details of CHAIN_ID computation here - https://github.com/ethereum/EIPs/blob/master/EIPS/eip-155.md#specification
        let (chain_id, rec_id) = match self.v {
            0..=26 | 29..=34 => return Err(Error::InvalidV),
            27..=28 => (
                None,
                u8::try_from(self.v - 27).map_err(|_e| Error::InvalidV)?,
            ),
            _ => (
                Some((self.v - 35) / 2),
                u8::try_from((self.v - 35) % 2).map_err(|_e| Error::InvalidV)?,
            ),
        };
        self.transaction
            .rlp_append_unsigned(&mut rlp_stream, chain_id);
        let message_hash = sdk::keccak(rlp_stream.as_raw());
        ecrecover(message_hash, &super::vrs_to_arr(rec_id, self.r, self.s))
            .map_err(|_e| Error::EcRecover)
    }

    /// Returns chain id encoded in `v` parameter of the signature if that was done, otherwise None.
    pub fn chain_id(&self) -> Option<u64> {
        match self.v {
            0..=34 => None,
            _ => Some((self.v - 35) / 2),
        }
    }
}

impl Encodable for LegacyEthSignedTransaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(9);
        s.append(&self.transaction.nonce);
        s.append(&self.transaction.gas_price);
        s.append(&self.transaction.gas_limit);
        match self.transaction.to.as_ref() {
            None => s.append(&""),
            Some(address) => s.append(&address.raw()),
        };
        s.append(&self.transaction.value.raw());
        s.append(&self.transaction.data);
        s.append(&self.v);
        s.append(&self.r);
        s.append(&self.s);
    }
}

impl Decodable for LegacyEthSignedTransaction {
    fn decode(rlp: &Rlp<'_>) -> Result<Self, DecoderError> {
        if rlp.item_count() != Ok(9) {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }
        let nonce = rlp.val_at(0)?;
        let gas_price = rlp.val_at(1)?;
        let gas = rlp.val_at(2)?;
        let to = super::rlp_extract_to(rlp, 3)?;
        let value = Wei::new(rlp.val_at(4)?);
        let data = rlp.val_at(5)?;
        let v = rlp.val_at(6)?;
        let r = rlp.val_at(7)?;
        let s = rlp.val_at(8)?;
        Ok(Self {
            transaction: TransactionLegacy {
                nonce,
                gas_price,
                gas_limit: gas,
                to,
                value,
                data,
            },
            v,
            r,
            s,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurora_engine_types::vec;

    #[test]
    fn test_eth_signed_no_chain_sender() {
        let encoded_tx = hex::decode("f901f680883362396163613030836691b78080b901a06080604052600080546001600160a01b0319163317905534801561002257600080fd5b5061016e806100326000396000f3fe608060405234801561001057600080fd5b50600436106100415760003560e01c8063445df0ac146100465780638da5cb5b14610060578063fdacd57614610084575b600080fd5b61004e6100a3565b60408051918252519081900360200190f35b6100686100a9565b604080516001600160a01b039092168252519081900360200190f35b6100a16004803603602081101561009a57600080fd5b50356100b8565b005b60015481565b6000546001600160a01b031681565b6000546001600160a01b031633146101015760405162461bcd60e51b81526004018080602001828103825260338152602001806101076033913960400191505060405180910390fd5b60015556fe546869732066756e6374696f6e206973207265737472696374656420746f2074686520636f6e74726163742773206f776e6572a265627a7a72315820b7e3396b30da5009ea603d5c2bdfd68577b979d5817fbe4fbd7d983f5c04ff3464736f6c634300050f00321ca0f0133510c01bc64a64f84b411082ff74bbc4a3aa5c720d2b5f61ad76716ee232a03412d91486eb012423492af258a4cd3b03ce67dde7fdc93bbea142bce6a59c9f").unwrap();
        let tx = LegacyEthSignedTransaction::decode(&Rlp::new(&encoded_tx)).unwrap();
        assert_eq!(tx.v, 28);
        assert_eq!(tx.chain_id(), None);
        assert_eq!(
            tx.sender().unwrap(),
            address_from_arr(&hex::decode("cf3c4c291ce0ad0ef5f6de577cd19d6d6ecf4db6").unwrap())
        );
    }

    #[test]
    fn test_decode_eth_signed_transaction() {
        let encoded_tx = hex::decode("f86a8086d55698372431831e848094f0109fc8df283027b6285cc889f5aa624eac1f55843b9aca008025a009ebb6ca057a0535d6186462bc0b465b561c94a295bdb0621fc19208ab149a9ca0440ffd775ce91a833ab410777204d5341a6f9fa91216a6f3ee2c051fea6a0428").unwrap();
        let tx = LegacyEthSignedTransaction::decode(&Rlp::new(&encoded_tx)).unwrap();
        assert_eq!(tx.v, 37);
        assert_eq!(tx.chain_id(), Some(1));
        assert_eq!(
            tx.transaction,
            TransactionLegacy {
                nonce: U256::zero(),
                gas_price: U256::from(234567897654321u128),
                gas_limit: U256::from(2000000u128),
                to: Some(address_from_arr(
                    &hex::decode("F0109fC8DF283027b6285cc889F5aA624EaC1F55").unwrap()
                )),
                value: Wei::new_u64(1000000000),
                data: vec![],
            }
        );
        assert_eq!(
            tx.sender().unwrap(),
            address_from_arr(&hex::decode("2c7536e3605d9c16a7a3d7b1898e529396a65c23").unwrap())
        );
    }

    #[test]
    fn test_none_decode_eth_signed_transaction() {
        let encoded_tx = hex::decode("f8c58001831e84808080b874600060005560648060106000396000f360e060020a6000350480638ada066e146028578063d09de08a1460365780632baeceb714604d57005b5060005460005260206000f3005b5060016000540160005560005460005260206000f3005b5060016000540360005560005460005260206000f300849c8a82cba07bea58c20d614248f6f1607704ee209eee14190f6187d6c7dc935b6599199cd5a02fe682dec51911f02d6a2812f301419d3f181acd3ef5b3609ac28b1dc42b0531").unwrap();
        let tx_1 = LegacyEthSignedTransaction::decode(&Rlp::new(&encoded_tx)).unwrap();

        let encoded_tx = hex::decode("f8d98001831e848094000000000000000000000000000000000000000080b874600060005560648060106000396000f360e060020a6000350480638ada066e146028578063d09de08a1460365780632baeceb714604d57005b5060005460005260206000f3005b5060016000540160005560005460005260206000f3005b5060016000540360005560005460005260206000f300849c8a82cba0668cfa20c8521b28fa8e42f26df0f2c090dda2fb5cbbb60dd616e8d00f93d9d8a00a1e5de8454ce9072cefd8268c0bf8eba2c1206a5e5a43914c1d62962c121d94").unwrap();
        let tx_2 = LegacyEthSignedTransaction::decode(&Rlp::new(&encoded_tx)).unwrap();

        // tx_2 has the zero address as its `to` field
        assert_eq!(tx_2.transaction.to, Some(address_from_arr(&[0u8; 20])));

        // otherwise, tx_1 and tx_2 are identical.
        let mut tx_2_mod = tx_2;
        tx_2_mod.transaction.to = None;
        assert_eq!(tx_1.transaction, tx_2_mod.transaction);
    }

    fn address_from_arr(arr: &[u8]) -> Address {
        assert_eq!(arr.len(), 20);
        Address::try_from_slice(arr).unwrap()
    }
}

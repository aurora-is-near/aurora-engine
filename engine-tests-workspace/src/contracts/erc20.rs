use crate::prelude::{transactions::legacy::TransactionLegacy, Address, U256};
use crate::test_utils::solidity;
use aurora_engine_transactions::NormalizedEthTransaction;
use ethabi::Bytes;
use std::path::{Path, PathBuf};

pub(crate) struct ERC20Constructor(pub solidity::ContractConstructor);

pub(crate) struct ERC20(pub solidity::DeployedContract);

impl From<ERC20Constructor> for solidity::ContractConstructor {
    fn from(c: ERC20Constructor) -> Self {
        c.0
    }
}

impl ERC20Constructor {
    pub fn load() -> Self {
        Self(solidity::ContractConstructor::compile_from_source(
            Self::solidity_sources_path(),
            Self::solidity_artifacts_path(),
            "token/ERC20/presets/ERC20PresetMinterPauser.sol",
            "ERC20PresetMinterPauser",
        ))
    }

    pub fn deploy(&self, name: &str, symbol: &str) -> Bytes {
        self.0
            .abi
            .constructor()
            .unwrap()
            .encode_input(
                self.0.code.clone(),
                &[
                    ethabi::Token::String(name.to_string()),
                    ethabi::Token::String(symbol.to_string()),
                ],
            )
            .unwrap()
    }

    fn solidity_sources_path() -> PathBuf {
        Path::new("etc")
            .join("openzeppelin-contracts")
            .join("contracts")
    }

    fn solidity_artifacts_path() -> PathBuf {
        Path::new("target").join("solidity-build")
    }
}

impl ERC20 {
    pub fn mint(&self, recipient: Address, amount: U256, nonce: U256) -> TransactionLegacy {
        let data = self
            .0
            .abi
            .function("mint")
            .unwrap()
            .encode_input(&[
                ethabi::Token::Address(recipient.raw()),
                ethabi::Token::Uint(amount),
            ])
            .unwrap();

        TransactionLegacy {
            nonce,
            gas_price: Default::default(),
            gas_limit: u64::MAX.into(),
            to: Some(self.0.address),
            value: Default::default(),
            data,
        }
    }

    pub fn transfer(&self, recipient: Address, amount: U256, nonce: U256) -> TransactionLegacy {
        let data = self
            .0
            .abi
            .function("transfer")
            .unwrap()
            .encode_input(&[
                ethabi::Token::Address(recipient.raw()),
                ethabi::Token::Uint(amount),
            ])
            .unwrap();
        TransactionLegacy {
            nonce,
            gas_price: Default::default(),
            gas_limit: u64::MAX.into(),
            to: Some(self.0.address),
            value: Default::default(),
            data,
        }
    }

    pub fn transfer_from(
        &self,
        from: Address,
        to: Address,
        amount: U256,
        nonce: U256,
    ) -> TransactionLegacy {
        let data = self
            .0
            .abi
            .function("transferFrom")
            .unwrap()
            .encode_input(&[
                ethabi::Token::Address(from.raw()),
                ethabi::Token::Address(to.raw()),
                ethabi::Token::Uint(amount),
            ])
            .unwrap();
        TransactionLegacy {
            nonce,
            gas_price: Default::default(),
            gas_limit: u64::MAX.into(),
            to: Some(self.0.address),
            value: Default::default(),
            data,
        }
    }

    pub fn approve(&self, spender: Address, amount: U256, nonce: U256) -> TransactionLegacy {
        let data = self
            .0
            .abi
            .function("approve")
            .unwrap()
            .encode_input(&[
                ethabi::Token::Address(spender.raw()),
                ethabi::Token::Uint(amount),
            ])
            .unwrap();
        TransactionLegacy {
            nonce,
            gas_price: Default::default(),
            gas_limit: u64::MAX.into(),
            to: Some(self.0.address),
            value: Default::default(),
            data,
        }
    }

    pub fn balance_of(&self, address: Address, nonce: U256) -> TransactionLegacy {
        let data = self
            .0
            .abi
            .function("balanceOf")
            .unwrap()
            .encode_input(&[ethabi::Token::Address(address.raw())])
            .unwrap();
        TransactionLegacy {
            nonce,
            gas_price: Default::default(),
            gas_limit: u64::MAX.into(),
            to: Some(self.0.address),
            value: Default::default(),
            data,
        }
    }
}

pub(crate) fn legacy_into_normalized_tx(tx: TransactionLegacy) -> NormalizedEthTransaction {
    NormalizedEthTransaction {
        address: Default::default(),
        chain_id: None,
        nonce: tx.nonce,
        gas_limit: tx.gas_limit,
        max_priority_fee_per_gas: tx.gas_price,
        max_fee_per_gas: tx.gas_price,
        to: tx.to,
        value: tx.value,
        data: tx.data,
        access_list: Vec::new(),
    }
}

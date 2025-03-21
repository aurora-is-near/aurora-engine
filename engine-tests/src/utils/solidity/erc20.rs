use crate::prelude::{transactions::legacy::TransactionLegacy, Address, U256};
use crate::utils::solidity;
use aurora_engine_transactions::NormalizedEthTransaction;
use aurora_engine_types::types::Wei;
use std::path::{Path, PathBuf};
use std::sync::Once;

pub struct ERC20Constructor(pub solidity::ContractConstructor);

pub struct ERC20(pub solidity::DeployedContract);

impl From<ERC20Constructor> for solidity::ContractConstructor {
    fn from(c: ERC20Constructor) -> Self {
        c.0
    }
}

static DOWNLOAD_ONCE: Once = Once::new();

impl ERC20Constructor {
    pub fn load() -> Self {
        Self(solidity::ContractConstructor::compile_from_source(
            Self::download_solidity_sources(),
            Self::solidity_artifacts_path(),
            "token/ERC20/presets/ERC20PresetMinterPauser.sol",
            "ERC20PresetMinterPauser",
        ))
    }

    pub fn deploy(&self, name: &str, symbol: &str, nonce: U256) -> TransactionLegacy {
        let data = self
            .0
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
            .unwrap();
        TransactionLegacy {
            nonce,
            gas_price: U256::default(),
            gas_limit: u64::MAX.into(),
            to: None,
            value: Wei::default(),
            data,
        }
    }

    fn download_solidity_sources() -> PathBuf {
        let sources_dir = Path::new("target").join("openzeppelin-contracts");
        let contracts_dir = sources_dir.join("contracts");

        if !contracts_dir.exists() {
            // Contracts not already present, so download them (but only once, even
            // if multiple tests running in parallel saw `contracts_dir` does not exist).
            DOWNLOAD_ONCE.call_once(|| {
                let url = "https://github.com/OpenZeppelin/openzeppelin-contracts";
                let repo = git2::Repository::clone(url, sources_dir).unwrap();
                // We need to checkout a specific commit hash because the preset contract we use
                // was removed from the repo later
                // (https://github.com/OpenZeppelin/openzeppelin-contracts/pull/3637).
                let commit_hash =
                    git2::Oid::from_str("dfef6a68ee18dbd2e1f5a099061a3b8a0e404485").unwrap();
                repo.set_head_detached(commit_hash).unwrap();
                let mut opts = git2::build::CheckoutBuilder::new();
                repo.checkout_head(Some(opts.force())).unwrap();
            });
        }

        contracts_dir
    }

    fn solidity_artifacts_path() -> PathBuf {
        Path::new("target").join("solidity_build")
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
                ethabi::Token::Address(recipient.raw().0.into()),
                ethabi::Token::Uint(ethabi::Uint::from(amount.to_big_endian())),
            ])
            .unwrap();

        TransactionLegacy {
            nonce,
            gas_price: U256::default(),
            gas_limit: u64::MAX.into(),
            to: Some(self.0.address),
            value: Wei::default(),
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
                ethabi::Token::Address(recipient.raw().0.into()),
                ethabi::Token::Uint(amount.to_big_endian().into()),
            ])
            .unwrap();
        TransactionLegacy {
            nonce,
            gas_price: U256::default(),
            gas_limit: u64::MAX.into(),
            to: Some(self.0.address),
            value: Wei::default(),
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
                ethabi::Token::Address(from.raw().0.into()),
                ethabi::Token::Address(to.raw().0.into()),
                ethabi::Token::Uint(amount.to_big_endian().into()),
            ])
            .unwrap();
        TransactionLegacy {
            nonce,
            gas_price: U256::default(),
            gas_limit: u64::MAX.into(),
            to: Some(self.0.address),
            value: Wei::default(),
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
                ethabi::Token::Address(spender.raw().0.into()),
                ethabi::Token::Uint(amount.to_big_endian().into()),
            ])
            .unwrap();
        TransactionLegacy {
            nonce,
            gas_price: U256::default(),
            gas_limit: u64::MAX.into(),
            to: Some(self.0.address),
            value: Wei::default(),
            data,
        }
    }

    pub fn balance_of(&self, address: Address, nonce: U256) -> TransactionLegacy {
        let data = self
            .0
            .abi
            .function("balanceOf")
            .unwrap()
            .encode_input(&[ethabi::Token::Address(address.raw().0.into())])
            .unwrap();
        TransactionLegacy {
            nonce,
            gas_price: U256::default(),
            gas_limit: u64::MAX.into(),
            to: Some(self.0.address),
            value: Wei::default(),
            data,
        }
    }
}

pub fn legacy_into_normalized_tx(tx: TransactionLegacy) -> NormalizedEthTransaction {
    NormalizedEthTransaction {
        address: Address::default(),
        chain_id: None,
        nonce: tx.nonce,
        gas_limit: tx.gas_limit,
        max_priority_fee_per_gas: tx.gas_price,
        max_fee_per_gas: tx.gas_price,
        to: tx.to,
        value: tx.value,
        data: tx.data,
        access_list: Vec::new(),
        authorization_list: Vec::new(),
    }
}

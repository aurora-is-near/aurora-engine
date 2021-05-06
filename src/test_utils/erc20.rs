use crate::prelude::{Address, U256};
use crate::test_utils::solidity;
use crate::transaction::EthTransaction;
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
            Self::download_solidity_sources(),
            Self::solidity_artifacts_path(),
            "token/ERC20/presets/ERC20PresetMinterPauser.sol",
            "ERC20PresetMinterPauser",
        ))
    }

    pub fn deploy(&self, name: &str, symbol: &str, nonce: U256) -> EthTransaction {
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
        EthTransaction {
            nonce,
            gas_price: Default::default(),
            gas: Default::default(),
            to: None,
            value: Default::default(),
            data,
        }
    }

    fn download_solidity_sources() -> PathBuf {
        let sources_dir = Path::new("target").join("openzeppelin-contracts");
        let contracts_dir = sources_dir.join("contracts");
        if contracts_dir.exists() {
            contracts_dir
        } else {
            let url = "https://github.com/OpenZeppelin/openzeppelin-contracts";
            let repo = git2::Repository::clone(url, sources_dir).unwrap();
            // repo.path() gives the path of the .git directory, so we need to use the parent
            repo.path().parent().unwrap().join("contracts")
        }
    }

    fn solidity_artifacts_path() -> PathBuf {
        Path::new("target").join("solidity_build")
    }
}

impl ERC20 {
    pub fn mint(&self, recipient: Address, amount: U256, nonce: U256) -> EthTransaction {
        let data = self
            .0
            .abi
            .function("mint")
            .unwrap()
            .encode_input(&[
                ethabi::Token::Address(recipient),
                ethabi::Token::Uint(amount),
            ])
            .unwrap();
        EthTransaction {
            nonce,
            gas_price: Default::default(),
            gas: Default::default(),
            to: Some(self.0.address),
            value: Default::default(),
            data,
        }
    }

    pub fn transfer(&self, recipient: Address, amount: U256, nonce: U256) -> EthTransaction {
        let data = self
            .0
            .abi
            .function("transfer")
            .unwrap()
            .encode_input(&[
                ethabi::Token::Address(recipient),
                ethabi::Token::Uint(amount),
            ])
            .unwrap();
        EthTransaction {
            nonce,
            gas_price: Default::default(),
            gas: Default::default(),
            to: Some(self.0.address),
            value: Default::default(),
            data,
        }
    }
}

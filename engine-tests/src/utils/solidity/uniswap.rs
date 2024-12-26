use crate::prelude::{Address, U256};
use crate::utils::solidity;
use aurora_engine_transactions::legacy::TransactionLegacy;
use aurora_engine_types::types::Wei;
use std::ops::Not;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

pub struct FactoryConstructor(pub solidity::ContractConstructor);

pub struct Factory(pub solidity::DeployedContract);

pub struct Pool(pub solidity::DeployedContract);

pub struct PositionManagerConstructor(pub solidity::ContractConstructor);

pub struct PositionManager(pub solidity::DeployedContract);

pub struct SwapRouterConstructor(pub solidity::ContractConstructor);

pub struct SwapRouter(pub solidity::DeployedContract);

pub struct MintParams {
    pub token0: Address,
    pub token1: Address,
    pub fee: u64,
    pub tick_lower: i64,
    pub tick_upper: i64,
    pub amount0_desired: U256,
    pub amount1_desired: U256,
    pub amount0_min: U256,
    pub amount1_min: U256,
    pub recipient: Address,
    pub deadline: U256,
}

impl From<FactoryConstructor> for solidity::ContractConstructor {
    fn from(c: FactoryConstructor) -> Self {
        c.0
    }
}

impl From<PositionManagerConstructor> for solidity::ContractConstructor {
    fn from(c: PositionManagerConstructor) -> Self {
        c.0
    }
}

impl From<SwapRouterConstructor> for solidity::ContractConstructor {
    fn from(c: SwapRouterConstructor) -> Self {
        c.0
    }
}

static DOWNLOAD_ONCE: Once = Once::new();

impl FactoryConstructor {
    pub fn load() -> Self {
        let artifact_path = uniswap_root_path().join(
            [
                "node_modules",
                "@uniswap",
                "v3-core",
                "artifacts",
                "contracts",
                "UniswapV3Factory.sol",
                "UniswapV3Factory.json",
            ]
            .iter()
            .collect::<PathBuf>(),
        );

        Self(load_constructor(artifact_path))
    }

    pub fn deploy(&self, nonce: U256) -> TransactionLegacy {
        self.0.deploy_without_args(nonce)
    }
}

impl PositionManagerConstructor {
    pub fn load() -> Self {
        let artifact_path = uniswap_root_path().join(
            [
                "node_modules",
                "@uniswap",
                "v3-periphery",
                "artifacts",
                "contracts",
                "NonfungiblePositionManager.sol",
                "NonfungiblePositionManager.json",
            ]
            .iter()
            .collect::<PathBuf>(),
        );

        Self(load_constructor(artifact_path))
    }

    pub fn deploy(
        &self,
        factory: Address,
        wrapped_eth: Address,
        token_descriptor: Address,
        nonce: U256,
    ) -> TransactionLegacy {
        let data = self
            .0
            .abi
            .constructor()
            .unwrap()
            .encode_input(
                self.0.code.clone(),
                &[
                    ethabi::Token::Address(ethabi::Address::from(factory.raw().0)),
                    ethabi::Token::Address(ethabi::Address::from(wrapped_eth.raw().0)),
                    ethabi::Token::Address(ethabi::Address::from(token_descriptor.raw().0)),
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
}

impl Factory {
    pub fn create_pool(
        &self,
        token_a: Address,
        token_b: Address,
        fee: U256,
        nonce: U256,
    ) -> TransactionLegacy {
        let data = self
            .0
            .abi
            .function("createPool")
            .unwrap()
            .encode_input(&[
                ethabi::Token::Address(ethabi::Address::from(token_a.raw().0)),
                ethabi::Token::Address(ethabi::Address::from(token_b.raw().0)),
                ethabi::Token::Uint(ethabi::Uint::from(fee.to_big_endian())),
            ])
            .unwrap();

        TransactionLegacy {
            nonce,
            gas_price: U256::zero(),
            gas_limit: u64::MAX.into(),
            to: Some(self.0.address),
            value: Wei::zero(),
            data,
        }
    }
}

impl Pool {
    pub fn from_address(address: Address) -> Self {
        let artifact_path = uniswap_root_path().join(
            [
                "node_modules",
                "@uniswap",
                "v3-core",
                "artifacts",
                "contracts",
                "UniswapV3Pool.sol",
                "UniswapV3Pool.json",
            ]
            .iter()
            .collect::<PathBuf>(),
        );
        let constructor = load_constructor(artifact_path);

        Self(solidity::DeployedContract {
            abi: constructor.abi,
            address,
        })
    }

    pub fn initialize(&self, price: U256, nonce: U256) -> TransactionLegacy {
        let data = self
            .0
            .abi
            .function("initialize")
            .unwrap()
            .encode_input(&[ethabi::Token::Uint(ethabi::Uint::from(
                price.to_big_endian(),
            ))])
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

impl PositionManager {
    pub fn mint(&self, params: &MintParams, nonce: U256) -> TransactionLegacy {
        let tick_lower = Self::i64_to_u256(params.tick_lower);
        let tick_upper = Self::i64_to_u256(params.tick_upper);
        let data = self
            .0
            .abi
            .function("mint")
            .unwrap()
            .encode_input(&[ethabi::Token::Tuple(vec![
                ethabi::Token::Address(ethabi::Address::from(params.token0.raw().0)),
                ethabi::Token::Address(ethabi::Address::from(params.token1.raw().0)),
                ethabi::Token::Uint(params.fee.into()),
                ethabi::Token::Int(ethabi::Int::from(tick_lower.to_big_endian())),
                ethabi::Token::Int(ethabi::Int::from(tick_upper.to_big_endian())),
                ethabi::Token::Uint(ethabi::Uint::from(params.amount0_desired.to_big_endian())),
                ethabi::Token::Uint(ethabi::Uint::from(params.amount1_desired.to_big_endian())),
                ethabi::Token::Uint(ethabi::Uint::from(params.amount0_min.to_big_endian())),
                ethabi::Token::Uint(ethabi::Uint::from(params.amount1_min.to_big_endian())),
                ethabi::Token::Address(ethabi::Address::from(params.recipient.raw().0)),
                ethabi::Token::Uint(ethabi::Uint::from(params.deadline.to_big_endian())),
            ])])
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

    fn i64_to_u256(x: i64) -> U256 {
        let y = U256::from(x.abs());
        if x < 0 {
            // compute two's complement to get negative number
            y.not().overflowing_add(U256::one()).0
        } else {
            y
        }
    }
}

impl SwapRouterConstructor {
    pub fn load() -> Self {
        let artifact_path = uniswap_root_path().join(
            [
                "node_modules",
                "@uniswap",
                "v3-periphery",
                "artifacts",
                "contracts",
                "SwapRouter.sol",
                "SwapRouter.json",
            ]
            .iter()
            .collect::<PathBuf>(),
        );

        Self(load_constructor(artifact_path))
    }

    pub fn deploy(&self, factory: Address, wrapped_eth: Address, nonce: U256) -> TransactionLegacy {
        let data = self
            .0
            .abi
            .constructor()
            .unwrap()
            .encode_input(
                self.0.code.clone(),
                &[
                    ethabi::Token::Address(ethabi::Address::from(factory.raw().0)),
                    ethabi::Token::Address(ethabi::Address::from(wrapped_eth.raw().0)),
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
}

pub struct ExactOutputSingleParams {
    pub token_in: Address,
    pub token_out: Address,
    pub fee: u64,
    pub recipient: Address,
    pub deadline: U256,
    pub amount_out: U256,
    pub amount_in_max: U256,
    pub price_limit: U256,
}

pub struct ExactInputParams {
    pub token_in: Address,
    // Vec of poolFee + tokenAddress
    pub path: Vec<(u64, Address)>,
    pub recipient: Address,
    pub deadline: U256,
    pub amount_in: U256,
    pub amount_out_min: U256,
}

impl SwapRouter {
    pub fn exact_output_single(
        &self,
        params: &ExactOutputSingleParams,
        nonce: U256,
    ) -> TransactionLegacy {
        let data = self
            .0
            .abi
            .function("exactOutputSingle")
            .unwrap()
            .encode_input(&[ethabi::Token::Tuple(vec![
                ethabi::Token::Address(ethabi::Address::from(params.token_in.raw().0)),
                ethabi::Token::Address(ethabi::Address::from(params.token_out.raw().0)),
                ethabi::Token::Uint(params.fee.into()),
                ethabi::Token::Address(ethabi::Address::from(params.recipient.raw().0)),
                ethabi::Token::Uint(ethabi::Uint::from(params.deadline.to_big_endian())),
                ethabi::Token::Uint(ethabi::Uint::from(params.amount_out.to_big_endian())),
                ethabi::Token::Uint(ethabi::Uint::from(params.amount_in_max.to_big_endian())),
                ethabi::Token::Uint(ethabi::Uint::from(params.price_limit.to_big_endian())),
            ])])
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

    pub fn exact_input(&self, params: &ExactInputParams, nonce: U256) -> TransactionLegacy {
        let path: Vec<u8> = {
            // The encoding here is 32-byte address, then 3-byte (24-bit) fee, alternating
            let mut result = Vec::with_capacity(32 + 35 * params.path.len());
            result.extend_from_slice(params.token_in.as_bytes());
            for (fee, token) in &params.path {
                let fee_bytes = fee.to_be_bytes();
                result.extend_from_slice(&fee_bytes[5..8]);
                result.extend_from_slice(token.as_bytes());
            }
            result
        };
        let data = self
            .0
            .abi
            .function("exactInput")
            .unwrap()
            .encode_input(&[ethabi::Token::Tuple(vec![
                ethabi::Token::Bytes(path),
                ethabi::Token::Address(ethabi::Address::from(params.recipient.raw().0)),
                ethabi::Token::Uint(ethabi::Uint::from(params.deadline.to_big_endian())),
                ethabi::Token::Uint(ethabi::Uint::from(params.amount_in.to_big_endian())),
                ethabi::Token::Uint(ethabi::Uint::from(params.amount_out_min.to_big_endian())),
            ])])
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

fn load_constructor(artifact_path: PathBuf) -> solidity::ContractConstructor {
    if !artifact_path.exists() {
        download_uniswap_artifacts();
    }

    solidity::ContractConstructor::compile_from_extended_json(artifact_path)
}

fn uniswap_root_path() -> PathBuf {
    Path::new("../etc").join("tests").join("uniswap")
}

fn download_uniswap_artifacts() {
    DOWNLOAD_ONCE.call_once(|| {
        let output = Command::new("/usr/bin/env")
            .current_dir(uniswap_root_path())
            .args(["yarn", "install"])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "Downloading uniswap npm package failed.\n{}",
            String::from_utf8(output.stderr).unwrap()
        );
    });
}

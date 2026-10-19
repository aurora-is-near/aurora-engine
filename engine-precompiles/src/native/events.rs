use crate::prelude::{H256, String, ToString, U256, types::Address, vec};

/// Derived from event signature (see `super::tests::test_exit_signatures`)
pub const EXIT_TO_NEAR_SIGNATURE: H256 = crate::make_h256(
    0x5a91b8bc9c1981673db8fb226dbd8fcd,
    0xd0c23f45cd28abb31403a5392f6dd0c7,
);
/// Derived from event signature (see `super::tests::test_exit_signatures`)
pub const EXIT_TO_ETH_SIGNATURE: H256 = crate::make_h256(
    0xd046c2bb01a5622bc4b9696332391d87,
    0x491373762eeac0831c48400e2d5a5f07,
);

/// The exit precompile events have an `erc20_address` field to indicate
/// which ERC-20 token is being withdrawn. However, ETH is not an ERC-20 token
/// So we need to have some other address to fill this field. This constant is
/// used for this purpose.
pub const ETH_ADDRESS: Address = Address::zero();

/// `ExitToNear`(
///    Address indexed sender,
///    Address indexed `erc20_address`,
///    string indexed dest,
///    uint amount
/// )
/// Note: in the ERC-20 exit case `sender` == `erc20_address` because it is
/// the ERC-20 contract which calls the exit precompile. However, in the case
/// of ETH exit the sender will give the true sender (and the `erc20_address`
/// will not be meaningful because ETH is not an ERC-20 token).
pub enum ExitToNear {
    Legacy(ExitToNearLegacy),
    Omni(ExitToNearOmni),
}

impl ExitToNear {
    #[must_use]
    pub fn encode(self) -> ethabi::RawLog {
        match self {
            Self::Legacy(legacy) => legacy.encode(),
            Self::Omni(omni) => omni.encode(),
        }
    }
}

pub struct ExitToNearLegacy {
    pub sender: Address,
    pub erc20_address: Address,
    pub dest: String,
    pub amount: U256,
}

impl ExitToNearLegacy {
    #[must_use]
    pub fn encode(self) -> ethabi::RawLog {
        let data = ethabi::encode(&[ethabi::Token::Uint(self.amount.to_big_endian().into())]);
        let topics = vec![
            EXIT_TO_NEAR_SIGNATURE.0.into(),
            encode_address(self.sender),
            encode_address(self.erc20_address),
            aurora_engine_sdk::keccak(&ethabi::encode(&[ethabi::Token::String(self.dest)]))
                .0
                .into(),
        ];

        ethabi::RawLog { topics, data }
    }
}

pub struct ExitToNearOmni {
    pub sender: Address,
    pub erc20_address: Address,
    pub dest: String,
    pub amount: U256,
    pub msg: String,
}

impl ExitToNearOmni {
    #[must_use]
    pub fn encode(self) -> ethabi::RawLog {
        let data = ethabi::encode(&[
            ethabi::Token::Uint(self.amount.to_big_endian().into()),
            ethabi::Token::String(self.msg),
        ]);
        let topics = vec![
            EXIT_TO_NEAR_SIGNATURE.0.into(),
            encode_address(self.sender),
            encode_address(self.erc20_address),
            aurora_engine_sdk::keccak(&ethabi::encode(&[ethabi::Token::String(self.dest)]))
                .0
                .into(),
        ];

        ethabi::RawLog { topics, data }
    }
}

#[must_use]
pub fn exit_to_near_schema() -> ethabi::Event {
    ethabi::Event {
        name: "ExitToNear".to_string(),
        inputs: vec![
            ethabi::EventParam {
                name: "sender".to_string(),
                kind: ethabi::ParamType::Address,
                indexed: true,
            },
            ethabi::EventParam {
                name: "erc20_address".to_string(),
                kind: ethabi::ParamType::Address,
                indexed: true,
            },
            ethabi::EventParam {
                name: "dest".to_string(),
                kind: ethabi::ParamType::String,
                indexed: true,
            },
            ethabi::EventParam {
                name: "amount".to_string(),
                kind: ethabi::ParamType::Uint(256),
                indexed: false,
            },
        ],
        anonymous: false,
    }
}

/// `ExitToEth`(
///    Address indexed sender,
///    Address indexed `erc20_address`,
///    Address indexed dest,
///    uint amount
/// )
/// Note: in the ERC-20 exit case `sender` == `erc20_address` because it is
/// the ERC-20 contract which calls the exit precompile. However, in the case
/// of ETH exit the sender will give the true sender (and the `erc20_address`
/// will not be meaningful because ETH is not an ERC-20 token).
pub struct ExitToEth {
    pub sender: Address,
    pub erc20_address: Address,
    pub dest: Address,
    pub amount: U256,
}

impl ExitToEth {
    #[must_use]
    pub fn encode(self) -> ethabi::RawLog {
        let data = ethabi::encode(&[ethabi::Token::Uint(self.amount.to_big_endian().into())]);
        let topics = vec![
            EXIT_TO_ETH_SIGNATURE.0.into(),
            encode_address(self.sender),
            encode_address(self.erc20_address),
            encode_address(self.dest),
        ];

        ethabi::RawLog { topics, data }
    }
}

#[must_use]
pub fn exit_to_eth_schema() -> ethabi::Event {
    ethabi::Event {
        name: "ExitToEth".to_string(),
        inputs: vec![
            ethabi::EventParam {
                name: "sender".to_string(),
                kind: ethabi::ParamType::Address,
                indexed: true,
            },
            ethabi::EventParam {
                name: "erc20_address".to_string(),
                kind: ethabi::ParamType::Address,
                indexed: true,
            },
            ethabi::EventParam {
                name: "dest".to_string(),
                kind: ethabi::ParamType::Address,
                indexed: true,
            },
            ethabi::EventParam {
                name: "amount".to_string(),
                kind: ethabi::ParamType::Uint(256),
                indexed: false,
            },
        ],
        anonymous: false,
    }
}

fn encode_address(a: Address) -> ethabi::Hash {
    let mut result = [0u8; 32];
    result[12..].copy_from_slice(a.as_bytes());
    result.into()
}

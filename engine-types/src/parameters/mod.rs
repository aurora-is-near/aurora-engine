use crate::{
    account_id::AccountId,
    types::{Address, NEP141Wei, NearGas, RawU256, Yocto},
    Box, String, Vec,
};
#[cfg(not(feature = "borsh-compat"))]
use borsh::{maybestd::io, BorshDeserialize, BorshSerialize};
#[cfg(feature = "borsh-compat")]
use borsh_compat::{self as borsh, maybestd::io, BorshDeserialize, BorshSerialize};

pub mod engine;

#[must_use]
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub enum PromiseArgs {
    Create(PromiseCreateArgs),
    Callback(PromiseWithCallbackArgs),
    Recursive(NearPromise),
}

impl PromiseArgs {
    /// Counts the total number of promises this call creates (including callbacks).
    #[must_use]
    pub fn promise_count(&self) -> u64 {
        match self {
            Self::Create(_) => 1,
            Self::Callback(_) => 2,
            Self::Recursive(p) => p.promise_count(),
        }
    }

    #[must_use]
    pub fn total_gas(&self) -> NearGas {
        match self {
            Self::Create(call) => call.attached_gas,
            Self::Callback(cb) => cb.base.attached_gas + cb.callback.attached_gas,
            Self::Recursive(p) => p.total_gas(),
        }
    }

    #[must_use]
    pub fn total_near(&self) -> Yocto {
        match self {
            Self::Create(call) => call.attached_balance,
            Self::Callback(cb) => cb.base.attached_balance + cb.callback.attached_balance,
            Self::Recursive(p) => p.total_near(),
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone, PartialEq, Eq)]
pub enum SimpleNearPromise {
    Create(PromiseCreateArgs),
    Batch(PromiseBatchAction),
}

impl SimpleNearPromise {
    #[must_use]
    pub fn total_gas(&self) -> NearGas {
        match self {
            Self::Create(call) => call.attached_gas,
            Self::Batch(batch) => {
                let total: u64 = batch
                    .actions
                    .iter()
                    .filter_map(|a| {
                        if let PromiseAction::FunctionCall { gas, .. } = a {
                            Some(gas.as_u64())
                        } else {
                            None
                        }
                    })
                    .sum();
                NearGas::new(total)
            }
        }
    }

    #[must_use]
    pub fn total_near(&self) -> Yocto {
        match self {
            Self::Create(call) => call.attached_balance,
            Self::Batch(batch) => {
                let total: u128 = batch
                    .actions
                    .iter()
                    .filter_map(|a| match a {
                        PromiseAction::FunctionCall { attached_yocto, .. } => {
                            Some(attached_yocto.as_u128())
                        }
                        PromiseAction::Transfer { amount } => Some(amount.as_u128()),
                        _ => None,
                    })
                    .sum();
                Yocto::new(total)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NearPromise {
    Simple(SimpleNearPromise),
    Then {
        base: Box<NearPromise>,
        // Near doesn't allow arbitrary promises in the callback,
        // only simple calls to contracts or batches of actions.
        callback: SimpleNearPromise,
    },
    And(Vec<NearPromise>),
}

impl NearPromise {
    #[must_use]
    pub fn promise_count(&self) -> u64 {
        match self {
            Self::Simple(_) => 1,
            Self::Then { base, .. } => base.promise_count() + 1,
            Self::And(ps) => ps.iter().map(Self::promise_count).sum(),
        }
    }

    #[must_use]
    pub fn total_gas(&self) -> NearGas {
        match self {
            Self::Simple(x) => x.total_gas(),
            Self::Then { base, callback } => base.total_gas() + callback.total_gas(),
            Self::And(promises) => {
                let total = promises.iter().map(|p| p.total_gas().as_u64()).sum();
                NearGas::new(total)
            }
        }
    }

    #[must_use]
    pub fn total_near(&self) -> Yocto {
        match self {
            Self::Simple(x) => x.total_near(),
            Self::Then { base, callback } => base.total_near() + callback.total_near(),
            Self::And(promises) => {
                let total = promises.iter().map(|p| p.total_near().as_u128()).sum();
                Yocto::new(total)
            }
        }
    }
}

// Cannot use derive macro on recursive types, so we write it by hand
impl BorshSerialize for NearPromise {
    fn serialize<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Simple(x) => {
                writer.write_all(&[0x00])?;
                x.serialize(writer)
            }
            Self::Then { base, callback } => {
                writer.write_all(&[0x01])?;
                base.serialize(writer)?;
                callback.serialize(writer)
            }
            Self::And(promises) => {
                writer.write_all(&[0x02])?;
                promises.serialize(writer)
            }
        }
    }
}

#[cfg(not(feature = "borsh-compat"))]
impl BorshDeserialize for NearPromise {
    fn deserialize_reader<R: io::Read>(reader: &mut R) -> io::Result<Self> {
        let variant_byte = {
            let mut buf = [0u8; 1];
            reader.read_exact(&mut buf)?;
            buf[0]
        };
        match variant_byte {
            0x00 => {
                let inner = SimpleNearPromise::deserialize_reader(reader)?;
                Ok(NearPromise::Simple(inner))
            }
            0x01 => {
                let base = Self::deserialize_reader(reader)?;
                let callback = SimpleNearPromise::deserialize_reader(reader)?;
                Ok(NearPromise::Then {
                    base: Box::new(base),
                    callback,
                })
            }
            0x02 => {
                let promises: Vec<NearPromise> = Vec::deserialize_reader(reader)?;
                Ok(NearPromise::And(promises))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid variant byte for NearPromise",
            )),
        }
    }
}

#[cfg(feature = "borsh-compat")]
impl BorshDeserialize for NearPromise {
    fn deserialize(buf: &mut &[u8]) -> io::Result<Self> {
        let variant_byte = buf[0];
        *buf = &buf[1..];
        match variant_byte {
            0x00 => {
                let inner = SimpleNearPromise::deserialize(buf)?;
                Ok(Self::Simple(inner))
            }
            0x01 => {
                let base = Self::deserialize(buf)?;
                let callback = SimpleNearPromise::deserialize(buf)?;
                Ok(Self::Then {
                    base: Box::new(base),
                    callback,
                })
            }
            0x02 => {
                let promises: Vec<Self> = Vec::deserialize(buf)?;
                Ok(Self::And(promises))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid variant byte for NearPromise",
            )),
        }
    }
}

#[must_use]
#[derive(Debug, BorshSerialize, BorshDeserialize, Clone, PartialEq, Eq)]
pub struct PromiseCreateArgs {
    pub target_account_id: AccountId,
    pub method: String,
    pub args: Vec<u8>,
    pub attached_balance: Yocto,
    pub attached_gas: NearGas,
}

#[must_use]
#[derive(Debug, BorshSerialize, BorshDeserialize, Clone, PartialEq, Eq)]
pub struct PromiseWithCallbackArgs {
    pub base: PromiseCreateArgs,
    pub callback: PromiseCreateArgs,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone, PartialEq, Eq)]
pub enum PromiseAction {
    CreateAccount,
    Transfer {
        amount: Yocto,
    },
    DeployContract {
        code: Vec<u8>,
    },
    FunctionCall {
        name: String,
        args: Vec<u8>,
        attached_yocto: Yocto,
        gas: NearGas,
    },
    Stake {
        amount: Yocto,
        public_key: NearPublicKey,
    },
    AddFullAccessKey {
        public_key: NearPublicKey,
        nonce: u64,
    },
    AddFunctionCallKey {
        public_key: NearPublicKey,
        nonce: u64,
        allowance: Yocto,
        receiver_id: AccountId,
        function_names: String,
    },
    DeleteKey {
        public_key: NearPublicKey,
    },
    DeleteAccount {
        beneficiary_id: AccountId,
    },
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone, PartialEq, Eq)]
pub enum NearPublicKey {
    /// ed25519 public keys are 32 bytes
    Ed25519([u8; 32]),
    /// secp256k1 keys are in the uncompressed 64 byte format
    Secp256k1([u8; 64]),
}

#[must_use]
#[derive(Debug, BorshSerialize, BorshDeserialize, Clone, PartialEq, Eq)]
pub struct PromiseBatchAction {
    pub target_account_id: AccountId,
    pub actions: Vec<PromiseAction>,
}

/// withdraw NEAR eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct WithdrawCallArgs {
    pub recipient_address: Address,
    pub amount: NEP141Wei,
}

/// withdraw NEAR eth-connector call args
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct RefundCallArgs {
    pub recipient_address: Address,
    pub erc20_address: Option<Address>,
    pub amount: RawU256,
}

/// Args passed to the the cross contract call precompile.
/// That precompile is used by Aurora contracts to make calls to the broader NEAR ecosystem.
/// See `https://github.com/aurora-is-near/AIPs/pull/2` for design details.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub enum CrossContractCallArgs {
    /// The promise is to be executed immediately (as part of the same NEAR transaction as the EVM call).
    Eager(PromiseArgs),
    /// The promise is to be stored in the router contract, and can be executed in a future transaction.
    /// The purpose of this is to expand how much NEAR gas can be made available to a cross contract call.
    /// For example, if an expensive EVM call ends with a NEAR cross contract call, then there may not be
    /// much gas left to perform it. In this case, the promise could be `Delayed` (stored in the router)
    /// and executed in a separate transaction with a fresh 300 Tgas available for it.
    Delayed(PromiseArgs),
}

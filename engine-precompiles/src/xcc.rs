//! Cross contract call precompile.
//!
//! Allow Aurora users interacting with NEAR smart contracts using cross contract call primitives.
//! TODO: How they work (low level explanation with examples)

use crate::{HandleBasedPrecompile, PrecompileOutput};
use aurora_engine_sdk::io::IO;
use aurora_engine_types::{
    account_id::AccountId,
    format,
    parameters::{CrossContractCallArgs, PromiseCreateArgs},
    types::{balance::ZERO_YOCTO, Address, EthGas, NearGas},
    vec, Cow, Vec, H160, H256, U256,
};
use borsh::{BorshDeserialize, BorshSerialize};
use evm::backend::Log;
use evm::executor::stack::{PrecompileFailure, PrecompileHandle};
use evm::ExitError;

pub mod costs {
    use crate::prelude::types::{EthGas, NearGas};

    /// Base EVM gas cost for calling this precompile.
    /// Value obtained from the following methodology:
    /// 1. Estimate the cost of calling this precompile in terms of NEAR gas.
    ///    This is done by calling the precompile with inputs of different lengths
    ///    and performing a linear regression to obtain a function
    ///    `NEAR_gas = CROSS_CONTRACT_CALL_BASE + (input_length) * (CROSS_CONTRACT_CALL_BYTE)`.
    /// 2. Convert the NEAR gas cost into an EVM gas cost using the conversion ratio below
    ///    (`CROSS_CONTRACT_CALL_NEAR_GAS`).
    ///
    /// This process is done in the `test_xcc_eth_gas_cost` test in
    /// `engine-tests/src/tests/xcc.rs`.
    pub const CROSS_CONTRACT_CALL_BASE: EthGas = EthGas::new(323_000);
    /// Additional EVM gas cost per bytes of input given.
    /// See `CROSS_CONTRACT_CALL_BASE` for estimation methodology.
    pub const CROSS_CONTRACT_CALL_BYTE: EthGas = EthGas::new(3);
    /// EVM gas cost per NEAR gas attached to the created promise.
    /// This value is derived from the gas report https://hackmd.io/@birchmd/Sy4piXQ29
    /// The units on this quantity are `NEAR Gas / EVM Gas`.
    /// The report gives a value `0.175 T(NEAR_gas) / k(EVM_gas)`. To convert the units to
    /// `NEAR Gas / EVM Gas`, we simply multiply `0.175 * 10^12 / 10^3 = 175 * 10^6`.
    pub const CROSS_CONTRACT_CALL_NEAR_GAS: u64 = 175_000_000;

    pub const ROUTER_EXEC_BASE: NearGas = NearGas::new(7_000_000_000_000);
    pub const ROUTER_EXEC_PER_CALLBACK: NearGas = NearGas::new(12_000_000_000_000);
    pub const ROUTER_SCHEDULE: NearGas = NearGas::new(5_000_000_000_000);
}

mod consts {
    pub(super) const ERR_INVALID_INPUT: &str = "ERR_INVALID_XCC_INPUT";
    pub(super) const ERR_SERIALIZE: &str = "ERR_XCC_CALL_SERIALIZE";
    pub(super) const ERR_STATIC: &str = "ERR_INVALID_IN_STATIC";
    pub(super) const ERR_DELEGATE: &str = "ERR_INVALID_IN_DELEGATE";
    pub(super) const ROUTER_EXEC_NAME: &str = "execute";
    pub(super) const ROUTER_SCHEDULE_NAME: &str = "schedule";
    /// Solidity selector for the ERC-20 transferFrom function
    /// https://www.4byte.directory/signatures/?bytes4_signature=0x23b872dd
    pub(super) const TRANSFER_FROM_SELECTOR: [u8; 4] = [0x23, 0xb8, 0x72, 0xdd];
}

pub struct CrossContractCall<I> {
    io: I,
    engine_account_id: AccountId,
}

impl<I> CrossContractCall<I> {
    pub fn new(engine_account_id: AccountId, io: I) -> Self {
        Self {
            io,
            engine_account_id,
        }
    }
}

pub mod cross_contract_call {
    use aurora_engine_types::{types::Address, H256};

    /// Exit to Ethereum precompile address
    ///
    /// Address: `0x516cded1d16af10cad47d6d49128e2eb7d27b372`
    /// This address is computed as: `&keccak("nearCrossContractCall")[12..]`
    pub const ADDRESS: Address =
        crate::make_address(0x516cded1, 0xd16af10cad47d6d49128e2eb7d27b372);

    /// Sentinel value used to indicate the following topic field is how much NEAR the
    /// cross-contract call will require.
    pub const AMOUNT_TOPIC: H256 =
        crate::make_h256(0x72657175697265645f6e656172, 0x72657175697265645f6e656172);
}

impl<I: IO> HandleBasedPrecompile for CrossContractCall<I> {
    fn run_with_handle(
        &self,
        handle: &mut impl PrecompileHandle,
    ) -> Result<PrecompileOutput, PrecompileFailure> {
        let input = handle.input();
        let target_gas = handle.gas_limit().map(EthGas::new);
        let context = handle.context();
        let is_static = handle.is_static();

        // This only includes the cost we can easily derive without parsing the input.
        // This allows failing fast without wasting computation on parsing.
        let input_len = u64::try_from(input.len()).map_err(crate::utils::err_usize_conv)?;
        let mut cost =
            costs::CROSS_CONTRACT_CALL_BASE + costs::CROSS_CONTRACT_CALL_BYTE * input_len;
        let check_cost = |cost: EthGas| -> Result<(), PrecompileFailure> {
            if let Some(target_gas) = target_gas {
                if cost > target_gas {
                    return Err(PrecompileFailure::Error {
                        exit_status: ExitError::OutOfGas,
                    });
                }
            }
            Ok(())
        };
        check_cost(cost)?;

        // It's not allowed to call cross contract call precompile in static or delegate mode
        if is_static {
            return Err(revert_with_message(consts::ERR_STATIC));
        } else if context.address != cross_contract_call::ADDRESS.raw() {
            return Err(revert_with_message(consts::ERR_DELEGATE));
        }

        let sender = context.caller;
        let target_account_id = create_target_account_id(sender, self.engine_account_id.as_ref());
        let args = CrossContractCallArgs::try_from_slice(input)
            .map_err(|_| ExitError::Other(Cow::from(consts::ERR_INVALID_INPUT)))?;
        let (promise, attached_near) = match args {
            CrossContractCallArgs::Eager(call) => {
                let call_gas = call.total_gas();
                let attached_near = call.total_near();
                let callback_count = call.promise_count() - 1;
                let router_exec_cost = costs::ROUTER_EXEC_BASE
                    + NearGas::new(callback_count * costs::ROUTER_EXEC_PER_CALLBACK.as_u64());
                let promise = PromiseCreateArgs {
                    target_account_id,
                    method: consts::ROUTER_EXEC_NAME.into(),
                    args: call
                        .try_to_vec()
                        .map_err(|_| ExitError::Other(Cow::from(consts::ERR_SERIALIZE)))?,
                    attached_balance: ZERO_YOCTO,
                    attached_gas: router_exec_cost + call_gas,
                };
                (promise, attached_near)
            }
            CrossContractCallArgs::Delayed(call) => {
                let attached_near = call.total_near();
                let promise = PromiseCreateArgs {
                    target_account_id,
                    method: consts::ROUTER_SCHEDULE_NAME.into(),
                    args: call
                        .try_to_vec()
                        .map_err(|_| ExitError::Other(Cow::from(consts::ERR_SERIALIZE)))?,
                    attached_balance: ZERO_YOCTO,
                    // We don't need to add any gas to the amount need for the schedule call
                    // since the promise is not executed right away.
                    attached_gas: costs::ROUTER_SCHEDULE,
                };
                (promise, attached_near)
            }
        };
        cost += EthGas::new(promise.attached_gas.as_u64() / costs::CROSS_CONTRACT_CALL_NEAR_GAS);
        check_cost(cost)?;

        let required_near =
            match state::get_code_version_of_address(&self.io, &Address::new(sender)) {
                // If there is no deployed version of the router contract then we need to charge for storage staking
                None => attached_near + state::STORAGE_AMOUNT,
                Some(_) => attached_near,
            };
        // if some NEAR payment is needed, transfer it from the caller to the engine's implicit address
        if required_near != ZERO_YOCTO {
            let engine_implicit_address = aurora_engine_sdk::types::near_account_to_evm_address(
                self.engine_account_id.as_bytes(),
            );
            let tx_data = transfer_from_args(
                sender,
                engine_implicit_address.raw(),
                U256::from(required_near.as_u128()),
            );
            let wnear_address = state::get_wnear_address(&self.io);
            let context = evm::Context {
                address: wnear_address.raw(),
                caller: cross_contract_call::ADDRESS.raw(),
                apparent_value: U256::zero(),
            };
            let (exit_reason, return_value) =
                handle.call(wnear_address.raw(), None, tx_data, None, false, &context);
            match exit_reason {
                // Transfer successful, nothing to do
                evm::ExitReason::Succeed(_) => (),
                evm::ExitReason::Revert(r) => {
                    return Err(PrecompileFailure::Revert {
                        exit_status: r,
                        output: return_value,
                    })
                }
                evm::ExitReason::Error(e) => {
                    return Err(PrecompileFailure::Error { exit_status: e })
                }
                evm::ExitReason::Fatal(f) => {
                    return Err(PrecompileFailure::Fatal { exit_status: f })
                }
            };
        }

        let topics = vec![
            cross_contract_call::AMOUNT_TOPIC,
            H256(aurora_engine_types::types::u256_to_arr(&U256::from(
                required_near.as_u128(),
            ))),
        ];

        let promise_log = Log {
            address: cross_contract_call::ADDRESS.raw(),
            topics,
            data: promise
                .try_to_vec()
                .map_err(|_| ExitError::Other(Cow::from(consts::ERR_SERIALIZE)))?,
        };

        Ok(PrecompileOutput {
            logs: vec![promise_log],
            cost,
            ..Default::default()
        })
    }
}

pub mod state {
    //! Functions for reading state related to the cross-contract call feature

    use aurora_engine_sdk::error::ReadU32Error;
    use aurora_engine_sdk::io::{StorageIntermediate, IO};
    use aurora_engine_types::storage::{self, KeyPrefix};
    use aurora_engine_types::types::{Address, Yocto};
    use borsh::{BorshDeserialize, BorshSerialize};

    pub const ERR_CORRUPTED_STORAGE: &str = "ERR_CORRUPTED_XCC_STORAGE";
    pub const ERR_MISSING_WNEAR_ADDRESS: &str = "ERR_MISSING_WNEAR_ADDRESS";
    pub const VERSION_KEY: &[u8] = b"version";
    pub const WNEAR_KEY: &[u8] = b"wnear";
    /// Amount of NEAR needed to cover storage for a router contract.
    pub const STORAGE_AMOUNT: Yocto = Yocto::new(2_000_000_000_000_000_000_000_000);

    /// Type wrapper for version of router contracts.
    #[derive(
        Debug,
        Clone,
        Copy,
        Default,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        BorshDeserialize,
        BorshSerialize,
    )]
    pub struct CodeVersion(pub u32);

    impl CodeVersion {
        pub fn increment(self) -> Self {
            Self(self.0 + 1)
        }
    }

    /// Get the address of the wNEAR ERC-20 contract
    pub fn get_wnear_address<I: IO>(io: &I) -> Address {
        let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, WNEAR_KEY);
        match io.read_storage(&key) {
            Some(bytes) => Address::try_from_slice(&bytes.to_vec()).expect(ERR_CORRUPTED_STORAGE),
            None => panic!("{}", ERR_MISSING_WNEAR_ADDRESS),
        }
    }

    /// Get the latest router contract version.
    pub fn get_latest_code_version<I: IO>(io: &I) -> CodeVersion {
        let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, VERSION_KEY);
        read_version(io, &key).unwrap_or_default()
    }

    /// Get the version of the currently deploy router for the given address (if it exists).
    pub fn get_code_version_of_address<I: IO>(io: &I, address: &Address) -> Option<CodeVersion> {
        let key = storage::bytes_to_key(KeyPrefix::CrossContractCall, address.as_bytes());
        read_version(io, &key)
    }

    /// Private utility method for reading code version from storage.
    fn read_version<I: IO>(io: &I, key: &[u8]) -> Option<CodeVersion> {
        match io.read_u32(key) {
            Ok(value) => Some(CodeVersion(value)),
            Err(ReadU32Error::MissingValue) => None,
            Err(ReadU32Error::InvalidU32) => panic!("{}", ERR_CORRUPTED_STORAGE),
        }
    }
}

fn transfer_from_args(from: H160, to: H160, amount: U256) -> Vec<u8> {
    let args = ethabi::encode(&[
        ethabi::Token::Address(from),
        ethabi::Token::Address(to),
        ethabi::Token::Uint(amount),
    ]);
    [&consts::TRANSFER_FROM_SELECTOR, args.as_slice()].concat()
}

fn create_target_account_id(sender: H160, engine_account_id: &str) -> AccountId {
    format!("{}.{}", hex::encode(sender.as_bytes()), engine_account_id)
        .parse()
        .unwrap()
}

fn revert_with_message(message: &str) -> PrecompileFailure {
    PrecompileFailure::Revert {
        exit_status: evm::ExitRevert::Reverted,
        output: message.as_bytes().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::sdk::types::near_account_to_evm_address;
    use crate::xcc::cross_contract_call;
    use aurora_engine_types::{vec, H160, U256};
    use rand::Rng;

    #[test]
    fn test_precompile_id() {
        assert_eq!(
            cross_contract_call::ADDRESS,
            near_account_to_evm_address("nearCrossContractCall".as_bytes())
        );
    }

    #[test]
    fn test_transfer_from_encoding() {
        let mut rng = rand::thread_rng();
        let from: [u8; 20] = rng.gen();
        let to: [u8; 20] = rng.gen();
        let amount: [u8; 32] = rng.gen();

        let from = H160(from);
        let to = H160(to);
        let amount = U256::from_big_endian(&amount);

        #[allow(deprecated)]
        let transfer_from_function = ethabi::Function {
            name: "transferFrom".into(),
            inputs: vec![
                ethabi::Param {
                    name: "from".into(),
                    kind: ethabi::ParamType::Address,
                    internal_type: None,
                },
                ethabi::Param {
                    name: "to".into(),
                    kind: ethabi::ParamType::Address,
                    internal_type: None,
                },
                ethabi::Param {
                    name: "amount".into(),
                    kind: ethabi::ParamType::Uint(256),
                    internal_type: None,
                },
            ],
            outputs: vec![ethabi::Param {
                name: "".into(),
                kind: ethabi::ParamType::Bool,
                internal_type: None,
            }],
            constant: None,
            state_mutability: ethabi::StateMutability::NonPayable,
        };
        let expected_tx_data = transfer_from_function
            .encode_input(&[
                ethabi::Token::Address(from),
                ethabi::Token::Address(to),
                ethabi::Token::Uint(amount),
            ])
            .unwrap();

        assert_eq!(
            super::transfer_from_args(from, to, amount),
            expected_tx_data
        );
    }
}

//! This is not a single precompile, but rather a "precompile template".
//! In particular, this is meant to replace the implementation of any ERC-20 contract
//! with an equivalent implementation in pure Rust. This will then be compiled to
//! wasm, and executed in the NEAR runtime directly, as opposed to the Solidity code
//! compiled to EVM which then runs in the EVM interpreter on top of wasm.
//! Therefore, this should be significantly more efficient than the EVM version.

use crate::{HandleBasedPrecompile, PrecompileOutput};
use aurora_engine_types::{types::EthGas, String, Vec, H160, H256, U256};
use evm::backend::Backend;
use evm::executor::stack::{PrecompileFailure, PrecompileHandle, StackState};
use evm::ExitRevert;

mod consts {
    pub(super) const TRANSFER_FROM_ARGS: &[ethabi::ParamType] = &[
        ethabi::ParamType::Address,
        ethabi::ParamType::Address,
        ethabi::ParamType::Uint(256),
    ];
    pub(super) const BALANCE_OF_ARGS: &[ethabi::ParamType] = &[ethabi::ParamType::Address];
    pub(super) const TRANSFER_ARGS: &[ethabi::ParamType] =
        &[ethabi::ParamType::Address, ethabi::ParamType::Uint(256)];
    pub(super) const ALLOWANCE_ARGS: &[ethabi::ParamType] =
        &[ethabi::ParamType::Address, ethabi::ParamType::Address];
    pub(super) const APPROVE_ARGS: &[ethabi::ParamType] =
        &[ethabi::ParamType::Address, ethabi::ParamType::Uint(256)];
    pub(super) const APPROVE_SELECTOR: &[u8] = &[0x09, 0x5e, 0xa7, 0xb3];
    pub(super) const BALANCE_OF_SELECTOR: &[u8] = &[0x70, 0xa0, 0x82, 0x31];
    pub(super) const TOTAL_SUPPLY_SELECTOR: &[u8] = &[0x18, 0x16, 0x0d, 0xdd];
    pub(super) const TRANSFER_SELECTOR: &[u8] = &[0xa9, 0x05, 0x9c, 0xbb];
    pub(super) const ALLOWANCE_SELECTOR: &[u8] = &[0xdd, 0x62, 0xed, 0x3e];
    pub(super) const TRANSFER_FROM_SELECTOR: &[u8] = &[0x23, 0xb8, 0x72, 0xdd];
    pub(super) const NAME_SELECTOR: &[u8] = &[0x06, 0xfd, 0xde, 0x03];
    pub(super) const SYMBOL_SELECTOR: &[u8] = &[0x95, 0xd8, 0x9b, 0x41];
    pub(super) const DECIMALS_SELECTOR: &[u8] = &[0x31, 0x3c, 0xe5, 0x67];
}

pub struct Erc20;

impl<'config> Erc20 {
    fn name(
        &self,
        handle: &mut impl PrecompileHandle<'config>,
    ) -> Result<PrecompileOutput, PrecompileFailure> {
        let name_key = slot(5);
        let address = handle.context().address;
        let name = read_as_string(handle.state_mut(), address, name_key);

        // TODO: cost?
        let cost = EthGas::new(0);
        Ok(PrecompileOutput {
            cost,
            output: ethabi::encode(&[ethabi::Token::String(name)]),
            logs: Vec::new(),
        })
    }

    fn symbol(
        &self,
        handle: &mut impl PrecompileHandle<'config>,
    ) -> Result<PrecompileOutput, PrecompileFailure> {
        let symbol_key = slot(6);
        let address = handle.context().address;
        let symbol = read_as_string(handle.state_mut(), address, symbol_key);

        // TODO: cost?
        let cost = EthGas::new(0);
        Ok(PrecompileOutput {
            cost,
            output: ethabi::encode(&[ethabi::Token::String(symbol)]),
            logs: Vec::new(),
        })
    }

    fn total_supply(
        &self,
        handle: &mut impl PrecompileHandle<'config>,
    ) -> Result<PrecompileOutput, PrecompileFailure> {
        let total_supply_key = slot(4);
        let address = handle.context().address;
        let total_supply = read_as_u256(handle.state_mut(), address, total_supply_key);

        // TODO: cost?
        let cost = EthGas::new(0);
        Ok(PrecompileOutput {
            cost,
            output: ethabi::encode(&[ethabi::Token::Uint(total_supply)]),
            logs: Vec::new(),
        })
    }

    fn balance_of(
        &self,
        handle: &mut impl PrecompileHandle<'config>,
        owner: &H160,
    ) -> Result<PrecompileOutput, PrecompileFailure> {
        let address = handle.context().address;
        let state = handle.state_mut();
        let balance_key = Self::create_balance_storage_key(owner);
        let balance = read_as_u256(state, address, balance_key);

        // TODO: cost?
        let cost = EthGas::new(0);
        Ok(PrecompileOutput {
            cost,
            output: ethabi::encode(&[ethabi::Token::Uint(balance)]),
            logs: Vec::new(),
        })
    }

    fn transfer(
        &self,
        handle: &mut impl PrecompileHandle<'config>,
        to: &H160,
        amount: U256,
    ) -> Result<PrecompileOutput, PrecompileFailure> {
        let (erc20_address, from) = {
            let ctx = handle.context();
            (ctx.address, ctx.caller)
        };
        let state = handle.state_mut();

        let balance_key = Self::create_balance_storage_key(&from);
        let current_balance = read_as_u256(state, erc20_address, balance_key);
        if current_balance < amount {
            // TODO: proper error message
            return Err(PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: Vec::new(),
            });
        }

        write_u256(state, erc20_address, balance_key, current_balance - amount);
        let balance_key = Self::create_balance_storage_key(to);
        // TODO: is this saturating add or checked add?
        write_u256(
            state,
            erc20_address,
            balance_key,
            read_as_u256(state, erc20_address, balance_key).saturating_add(amount),
        );

        // TODO: cost?
        let cost = EthGas::new(0);
        Ok(PrecompileOutput {
            cost,
            output: ethabi::encode(&[ethabi::Token::Bool(true)]),
            // TODO: proper event output
            logs: Vec::new(),
        })
    }

    fn allowance(
        &self,
        handle: &mut impl PrecompileHandle<'config>,
        owner: &H160,
        spender: &H160,
    ) -> Result<PrecompileOutput, PrecompileFailure> {
        let address = handle.context().address;
        let state = handle.state_mut();
        let allowance_key = Self::create_allowance_storage_key(owner, spender);
        let allowance = read_as_u256(state, address, allowance_key);

        // TODO: cost?
        let cost = EthGas::new(0);
        Ok(PrecompileOutput {
            cost,
            output: ethabi::encode(&[ethabi::Token::Uint(allowance)]),
            logs: Vec::new(),
        })
    }

    fn approve(
        &self,
        handle: &mut impl PrecompileHandle<'config>,
        spender: &H160,
        amount: U256,
    ) -> Result<PrecompileOutput, PrecompileFailure> {
        let (erc20_address, owner) = {
            let ctx = handle.context();
            (ctx.address, ctx.caller)
        };
        let state = handle.state_mut();

        let allowance_key = Self::create_allowance_storage_key(&owner, spender);
        // TODO: is this saturating add or checked add?
        write_u256(
            state,
            erc20_address,
            allowance_key,
            read_as_u256(state, erc20_address, allowance_key).saturating_add(amount),
        );

        // TODO: cost?
        let cost = EthGas::new(0);
        Ok(PrecompileOutput {
            cost,
            output: ethabi::encode(&[ethabi::Token::Bool(true)]),
            // TODO: proper event output
            logs: Vec::new(),
        })
    }

    fn transfer_from(
        &self,
        handle: &mut impl PrecompileHandle<'config>,
        from: &H160,
        to: &H160,
        amount: U256,
    ) -> Result<PrecompileOutput, PrecompileFailure> {
        let (erc20_address, spender) = {
            let ctx = handle.context();
            (ctx.address, ctx.caller)
        };
        let state = handle.state_mut();

        let allowance_key = Self::create_allowance_storage_key(from, &spender);
        let current_allowance = read_as_u256(state, erc20_address, allowance_key);
        if current_allowance < amount {
            // TODO: proper error message
            return Err(PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: Vec::new(),
            });
        }

        let balance_key = Self::create_balance_storage_key(from);
        let current_balance = read_as_u256(state, erc20_address, balance_key);
        if current_balance < amount {
            // TODO: proper error message
            return Err(PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: Vec::new(),
            });
        }

        if current_allowance != U256::MAX {
            write_u256(
                state,
                erc20_address,
                allowance_key,
                current_allowance - amount,
            );
        }
        write_u256(state, erc20_address, balance_key, current_balance - amount);
        let balance_key = Self::create_balance_storage_key(to);
        // TODO: is this saturating add or checked add?
        write_u256(
            state,
            erc20_address,
            balance_key,
            read_as_u256(state, erc20_address, balance_key).saturating_add(amount),
        );

        // TODO: cost?
        let cost = EthGas::new(0);
        Ok(PrecompileOutput {
            cost,
            output: ethabi::encode(&[ethabi::Token::Bool(true)]),
            // TODO: proper event output
            logs: Vec::new(),
        })
    }

    fn create_balance_storage_key(owner: &H160) -> H256 {
        let mut bytes = Vec::with_capacity(64);
        bytes.extend_from_slice(&[0u8; 12]);
        bytes.extend_from_slice(owner.as_bytes());
        bytes.extend_from_slice(&[0u8; 31]);
        bytes.push(2); // balance mapping is in "slot 2"

        aurora_engine_sdk::keccak(&bytes)
    }

    fn create_allowance_storage_key(owner: &H160, spender: &H160) -> H256 {
        let mut bytes = Vec::with_capacity(64);
        bytes.extend_from_slice(&[0u8; 12]);
        bytes.extend_from_slice(owner.as_bytes());
        bytes.extend_from_slice(&[0u8; 31]);
        bytes.push(3); // allowance mapping is in "slot 3"

        let hash1 = aurora_engine_sdk::keccak(&bytes);

        bytes.clear();
        bytes.extend_from_slice(&[0u8; 12]);
        bytes.extend_from_slice(spender.as_bytes());
        bytes.extend_from_slice(hash1.as_bytes());

        aurora_engine_sdk::keccak(&bytes)
    }
}

const fn slot(n: u8) -> H256 {
    let mut tmp = [0u8; 32];
    tmp[31] = n;
    H256(tmp)
}

fn read_as_string<S: Backend>(state: &S, address: H160, key: H256) -> String {
    let value = state.storage(address, key);

    if value.0[31] % 2 == 1 {
        panic!("Long format strings not implemented");
    }

    let length = usize::from(value.0[31] / 2);
    let bytes = &value.as_bytes()[0..length];
    // TODO: is lossy conversion fine here?
    String::from_utf8_lossy(bytes).into()
}

fn read_as_u256<S: Backend>(state: &S, address: H160, key: H256) -> U256 {
    U256::from_big_endian(state.storage(address, key).as_bytes())
}

fn write_u256<'a, S: StackState<'a>>(state: &mut S, address: H160, key: H256, value: U256) {
    let mut bytes = [0u8; 32];
    value.to_big_endian(&mut bytes);
    state.set_storage(address, key, H256(bytes))
}

impl<'config> HandleBasedPrecompile<'config> for Erc20 {
    fn run_with_handle(
        &self,
        handle: &mut impl PrecompileHandle<'config>,
    ) -> Result<PrecompileOutput, PrecompileFailure> {
        let input = handle.input();

        if input.len() < 4 {
            return Err(PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: Vec::new(),
            });
        }

        let selector = &input[0..4];
        match selector {
            consts::NAME_SELECTOR => self.name(handle),
            consts::SYMBOL_SELECTOR => self.symbol(handle),
            consts::DECIMALS_SELECTOR => {
                // TODO: cost
                Ok(PrecompileOutput::without_logs(
                    EthGas::new(0),
                    ethabi::encode(&[ethabi::Token::Uint(18.into())]),
                ))
            }
            consts::TOTAL_SUPPLY_SELECTOR => self.total_supply(handle),
            consts::BALANCE_OF_SELECTOR => {
                let parsed_args =
                    ethabi::decode(consts::BALANCE_OF_ARGS, &input[4..]).map_err(|_| {
                        PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: Vec::new(),
                        }
                    })?;
                let owner = as_address(&parsed_args[0]).unwrap();
                self.balance_of(handle, owner)
            }
            consts::TRANSFER_SELECTOR => {
                let parsed_args =
                    ethabi::decode(consts::TRANSFER_ARGS, &input[4..]).map_err(|_| {
                        PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: Vec::new(),
                        }
                    })?;
                let to = as_address(&parsed_args[0]).unwrap(); // unwrap is because of the types passed to `decode` above
                let amount = *as_uint(&parsed_args[1]).unwrap();
                self.transfer(handle, to, amount)
            }
            consts::ALLOWANCE_SELECTOR => {
                let parsed_args =
                    ethabi::decode(consts::ALLOWANCE_ARGS, &input[4..]).map_err(|_| {
                        PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: Vec::new(),
                        }
                    })?;
                let owner = as_address(&parsed_args[0]).unwrap();
                let spender = as_address(&parsed_args[1]).unwrap();
                self.allowance(handle, owner, spender)
            }
            consts::APPROVE_SELECTOR => {
                let parsed_args =
                    ethabi::decode(consts::APPROVE_ARGS, &input[4..]).map_err(|_| {
                        PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: Vec::new(),
                        }
                    })?;
                let spender = as_address(&parsed_args[0]).unwrap(); // unwrap is because of the types passed to `decode` above
                let amount = *as_uint(&parsed_args[1]).unwrap();
                self.approve(handle, spender, amount)
            }
            consts::TRANSFER_FROM_SELECTOR => {
                let parsed_args =
                    ethabi::decode(consts::TRANSFER_FROM_ARGS, &input[4..]).map_err(|_| {
                        PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: Vec::new(),
                        }
                    })?;
                let from = as_address(&parsed_args[0]).unwrap(); // unwrap is because of the types passed to `decode` above
                let to = as_address(&parsed_args[1]).unwrap();
                let amount = *as_uint(&parsed_args[2]).unwrap();
                self.transfer_from(handle, from, to, amount)
            }
            _ => Err(PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: Vec::new(),
            }),
        }
    }
}

fn as_address(token: &ethabi::Token) -> Option<&H160> {
    match token {
        ethabi::Token::Address(a) => Some(a),
        _ => None,
    }
}

fn as_uint(token: &ethabi::Token) -> Option<&U256> {
    match token {
        ethabi::Token::Uint(x) => Some(x),
        _ => None,
    }
}

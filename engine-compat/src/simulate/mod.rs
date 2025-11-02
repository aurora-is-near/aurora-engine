mod engine_state_override;

use alloc::{collections::BTreeMap, vec::Vec};

use aurora_engine::{
    engine::{self, Engine, EngineError, EngineErrorKind},
    parameters::SubmitResult,
};
use aurora_engine_modexp::{AuroraModExp, ModExpAlgorithm};
use aurora_engine_sdk::{
    env::Env,
    io::{StorageIntermediate, IO},
};
use aurora_engine_transactions::NormalizedEthTransaction;
use aurora_engine_types::{
    parameters::simulate::SimulateEthCallArgs,
    types::{Address, EthGas, GasLimit, Wei},
};
use primitive_types::U256;

use self::engine_state_override::StorageOverride;

#[derive(Debug, serde::Serialize, serde::Deserialize)]

pub enum StateOrEngineError {
    StateMissing,

    Engine(EngineError),
}

impl From<EngineError> for StateOrEngineError {
    fn from(value: EngineError) -> Self {
        Self::Engine(value)
    }
}

pub fn eth_call<I, E>(io: I, env: E) -> Result<SubmitResult, StateOrEngineError>
where
    I: IO + Send + Copy,
    E: Env,
{
    let SimulateEthCallArgs {
        from,
        to,
        gas_limit,
        gas_price,
        value,
        data,
        nonce,
        state_override,
    } = io
        .read_input()
        .to_value()
        .expect("Failed to deserialize EthCallInput");

    let current_nonce = engine::get_nonce(&io, &from).low_u64();
    let mut local_io = io;
    let mut full_override = BTreeMap::new();
    for (address, state_override) in state_override {
        if let Some(balance) = state_override.balance {
            engine::set_balance(&mut local_io, &address, &Wei::new(balance.into()));
        }
        if let Some(nonce) = state_override.nonce {
            engine::set_nonce(&mut local_io, &address, &nonce.into());
        }
        if let Some(code) = state_override.code {
            engine::set_code(&mut local_io, &address, &code);
        }
        if let Some(state) = state_override.state {
            full_override.insert(address.raw(), state);
        }
        if let Some(state_diff) = state_override.state_diff {
            let generation = engine::get_generation(&local_io, &address);
            for (k, v) in state_diff {
                engine::set_storage(&mut local_io, &address, &k.into(), &v.into(), generation);
            }
        }
    }

    // debug info nonce status: 1 -> not provided, 2 -> too low, 3 -> greater or equal
    let nonce_status = nonce.map_or(1u64, |nonce| if nonce < current_nonce { 2 } else { 3 });
    local_io.write_borsh(
        b"borealis/custom_debug_info",
        &(nonce_status, current_nonce),
    );

    if full_override.is_empty() {
        compute_call_result(
            local_io,
            env,
            from,
            to,
            gas_limit,
            gas_price.into(),
            value,
            data,
            nonce,
        )
    } else {
        let override_io = StorageOverride {
            inner: local_io,
            state_override: &full_override,
        };
        compute_call_result(
            override_io,
            env,
            from.into(),
            to,
            gas_limit,
            gas_price.into(),
            value,
            data,
            nonce,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn compute_call_result<I: IO + Copy, E: Env>(
    io: I,
    env: E,
    from: Address,
    to: Option<Address>,
    gas_limit: GasLimit,
    gas_price: U256,
    value: Wei,
    data: Vec<u8>,
    nonce: Option<u64>,
) -> Result<SubmitResult, StateOrEngineError> {
    let mut handler = aurora_engine_sdk::promise::Noop;
    aurora_engine::state::get_state(&io)
        .map_err(|_err| StateOrEngineError::StateMissing)
        .and_then(|engine_state| {
            let mut engine: Engine<_, _, AuroraModExp> = Engine::new_with_state(
                engine_state,
                from,
                env.current_account_id().clone(),
                io,
                &env,
            );
            let fixed_gas_cost = aurora_engine::contract_methods::silo::get_fixed_gas(&io);
            let user_gas_limit = gas_limit.unlimited_user_defined_value();
            // If the user provided a gas limit in their request then we can charge gas
            // before executing the transaction
            if !gas_price.is_zero() {
                if let Some(gas_limit) = user_gas_limit {
                    charge_gas(
                        &mut engine,
                        from,
                        to,
                        gas_limit,
                        gas_price,
                        value,
                        nonce,
                        fixed_gas_cost,
                    )?
                }
            }
            let result = match to {
                Some(to) => engine.call(
                    &from,
                    &to,
                    value,
                    data,
                    gas_limit.value(),
                    Vec::new(),
                    &mut handler,
                ),
                None => engine.deploy_code(
                    from,
                    value,
                    data,
                    None,
                    gas_limit.value(),
                    Vec::new(),
                    &mut handler,
                ),
            };
            // If the user did not provide a gas limit, but they did give a gas price
            // then we still charge gas based on the estimated by the call.
            if !gas_price.is_zero() && user_gas_limit.is_none() && result.is_ok() {
                let gas_used = result.as_ref().unwrap().gas_used;
                let gas_estimate = gas_used.saturating_add(gas_used / 3);
                charge_gas(
                    &mut engine,
                    from,
                    to,
                    gas_estimate,
                    gas_price,
                    value,
                    nonce,
                    fixed_gas_cost,
                )?
            }
            result.map_err(From::from)
        })
}

#[allow(clippy::too_many_arguments)]
fn charge_gas<I: IO + Copy, E: Env, M: ModExpAlgorithm>(
    engine: &mut Engine<I, E, M>,
    from: Address,
    to: Option<Address>,
    gas_limit: u64,
    gas_price: U256,
    value: Wei,
    nonce: Option<u64>,
    fixed_gas_cost: Option<EthGas>,
) -> Result<(), EngineError> {
    let transaction = NormalizedEthTransaction {
        address: from,
        chain_id: None,
        nonce: nonce.map(U256::from).unwrap_or_default(),
        gas_limit: U256::from(gas_limit),
        max_priority_fee_per_gas: gas_price,
        max_fee_per_gas: gas_price,
        to,
        value,
        // We do not use the real `data` here to avoid moving it before passing to `call`.
        // It is ok to not have the `data` here because it is not used by the `charge_gas` function.
        data: Vec::new(),
        access_list: Vec::new(),
    };
    engine
        .charge_gas(&from, &transaction, None, fixed_gas_cost)
        .map_err(|e| EngineError::from(EngineErrorKind::GasPayment(e)))?;
    Ok(())
}

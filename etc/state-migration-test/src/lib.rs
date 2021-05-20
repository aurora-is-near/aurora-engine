#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use aurora_engine::engine::{Engine, EngineState};
use aurora_engine::{sdk, storage};
use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshDeserialize, BorshSerialize)]
struct NewFancyState {
    old_state: EngineState,
    some_other_numbers: [u32; 7],
}

#[no_mangle]
pub extern "C" fn state_migration() {
    let old_state = match Engine::get_state() {
        Ok(state) => state,
        Err(e) => sdk::panic_utf8(e.as_ref()),
    };

    let new_state = NewFancyState {
        old_state,
        some_other_numbers: [3, 1, 4, 1, 5, 9, 2],
    };

    sdk::write_storage(&state_key(), &new_state.try_to_vec().expect("ERR_SER"));
}

#[no_mangle]
pub extern "C" fn some_new_fancy_function() {
    let state = sdk::read_storage(&state_key())
        .and_then(|bytes| NewFancyState::try_from_slice(&bytes).ok())
        .unwrap();

    sdk::return_output(&state.some_other_numbers.try_to_vec().unwrap());
}

fn state_key() -> Vec<u8> {
    storage::bytes_to_key(storage::KeyPrefix::Config, b"STATE")
}

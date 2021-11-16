#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use aurora_engine::engine::{self, EngineState};
use aurora_engine_sdk::near_runtime::Runtime;
use aurora_engine_sdk::io::{IO, StorageIntermediate};
use aurora_engine_types::storage;
use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshDeserialize, BorshSerialize)]
struct NewFancyState {
    old_state: EngineState,
    some_other_numbers: [u32; 7],
}

#[no_mangle]
pub extern "C" fn state_migration() {
    let mut io = Runtime;
    let old_state = match engine::get_state(&io) {
        Ok(state) => state,
        Err(e) => aurora_engine_sdk::panic_utf8(e.as_ref()),
    };

    let new_state = NewFancyState {
        old_state,
        some_other_numbers: [3, 1, 4, 1, 5, 9, 2],
    };

    io.write_storage(&state_key(), &new_state.try_to_vec().expect("ERR_SER"));
}

#[no_mangle]
pub extern "C" fn some_new_fancy_function() {
    let mut io = Runtime;
    let state = io.read_storage(&state_key())
        .and_then(|bytes| NewFancyState::try_from_slice(&bytes.to_vec()).ok())
        .unwrap();

    io.return_output(&state.some_other_numbers.try_to_vec().unwrap());
}

fn state_key() -> Vec<u8> {
    storage::bytes_to_key(storage::KeyPrefix::Config, b"STATE")
}

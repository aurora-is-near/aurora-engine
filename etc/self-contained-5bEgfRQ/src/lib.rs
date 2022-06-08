#![no_std]

extern crate alloc;

mod promise;

use aurora_engine_sdk::{
    env,
    io::{self, IO},
    near_runtime,
};
use aurora_engine_types::types::gas::NearGas;
use aurora_engine_types::{BTreeMap, Cow, Vec, H256};
use borsh::{BorshDeserialize, BorshSerialize};

const STATE: &[u8; 52072] = include_bytes!("../state.bin");
const INPUT: &[u8] = &[
    248, 109, 129, 203, 132, 1, 201, 195, 128, 131, 102, 145, 183, 148, 63, 113, 89, 149, 100, 127,
    228, 77, 180, 84, 17, 187, 158, 129, 183, 161, 173, 90, 131, 135, 128, 132, 188, 103, 232, 135,
    132, 156, 138, 130, 199, 160, 85, 99, 19, 114, 59, 112, 86, 130, 51, 7, 173, 56, 131, 100, 65,
    4, 75, 55, 195, 220, 143, 211, 226, 205, 75, 118, 39, 64, 248, 101, 95, 156, 160, 1, 115, 246,
    9, 137, 244, 94, 244, 29, 39, 85, 229, 80, 165, 155, 165, 17, 193, 95, 61, 221, 8, 35, 85, 138,
    5, 237, 53, 68, 18, 30, 90,
];

#[no_mangle]
pub extern "C" fn run() {
    let local_env = env::Fixed {
        signer_account_id: "relay.aurora".parse().unwrap(),
        current_account_id: "aurora".parse().unwrap(),
        predecessor_account_id: "relay.aurora".parse().unwrap(),
        block_height: 64417403,
        block_timestamp: env::Timestamp::new(1651073772931594646),
        attached_deposit: 0,
        random_seed: H256([0u8; 32]),
        prepaid_gas: NearGas::new(300_000_000_000_000),
    };

    let state = BTreeMap::try_from_slice(STATE).unwrap();
    let in_mem_io = InMemIO::new(&state, INPUT);

    let engine_state = aurora_engine::engine::get_state(&in_mem_io).unwrap();
    let relayer_address = aurora_engine_sdk::types::near_account_to_evm_address(
        local_env.predecessor_account_id.as_bytes(),
    );
    let mut handler = promise::Noop;
    let result = aurora_engine::engine::submit(
        in_mem_io,
        &local_env,
        INPUT,
        engine_state,
        local_env.current_account_id.clone(),
        relayer_address,
        &mut handler,
    )
    .unwrap();

    let mut rt = near_runtime::Runtime;
    let return_bytes = result.try_to_vec().unwrap();
    rt.return_output(&return_bytes);
}

#[derive(Clone, Copy)]
struct InMemIO<'a> {
    kv_store: &'a BTreeMap<Vec<u8>, Vec<u8>>,
    input_bytes: &'a [u8],
}

impl<'a> InMemIO<'a> {
    fn new(kv_store: &'a BTreeMap<Vec<u8>, Vec<u8>>, input_bytes: &'a [u8]) -> Self {
        Self {
            kv_store,
            input_bytes,
        }
    }
}

struct InMemIOStorageValue<'a>(Cow<'a, [u8]>);

impl<'a> io::StorageIntermediate for InMemIOStorageValue<'a> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn copy_to_slice(&self, buffer: &mut [u8]) {
        buffer.copy_from_slice(self.0.as_ref())
    }

    fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl<'a> IO for InMemIO<'a> {
    type StorageValue = InMemIOStorageValue<'a>;

    fn read_input(&self) -> Self::StorageValue {
        InMemIOStorageValue(Cow::Borrowed(self.input_bytes))
    }

    fn read_storage(&self, key: &[u8]) -> Option<Self::StorageValue> {
        self.kv_store
            .get(key)
            .map(|v| InMemIOStorageValue(Cow::Owned(v.clone())))
    }

    fn storage_has_key(&self, key: &[u8]) -> bool {
        self.kv_store.contains_key(key)
    }

    // The mutable methods are broken, but that's ok because we don't actually need to change this storage
    fn return_output(&mut self, _value: &[u8]) {}

    fn write_storage(&mut self, _key: &[u8], _value: &[u8]) -> Option<Self::StorageValue> {
        None
    }

    fn write_storage_direct(
        &mut self,
        _key: &[u8],
        _value: Self::StorageValue,
    ) -> Option<Self::StorageValue> {
        None
    }

    fn remove_storage(&mut self, _key: &[u8]) -> Option<Self::StorageValue> {
        None
    }
}

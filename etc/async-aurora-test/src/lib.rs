use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::near_bindgen;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Default)]
pub struct AsyncAuroraTest {
    value: i128,
}

#[near_bindgen]
impl AsyncAuroraTest {
    pub fn add(&mut self, arg: i128) {
        self.value += arg;
    }

    pub fn sub(&mut self, arg: i128) {
        self.value -= arg;
    }

    pub fn mul(&mut self, arg: i128) {
        self.value *= arg;
    }

    pub fn get_value(&self) -> i128 {
        self.value
    }
}
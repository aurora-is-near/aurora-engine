use crate::prelude::{Address, String, Vec, H256, U256};
use borsh::{BorshDeserialize, BorshSerialize};

#[cfg(not(feature = "contract"))]
use sha3::{Digest, Keccak256};

#[cfg(feature = "contract")]
use crate::sdk;

pub type AccountId = String;
pub type RawAddress = [u8; 20];
pub type RawU256 = [u8; 32]; // Little-endian large integer type.
pub type RawH256 = [u8; 32]; // Unformatted binary data of fixed length.
pub type Gas = u64;
pub type Balance = u128;

#[derive(BorshSerialize, BorshDeserialize)]
pub struct U128(pub u128);

pub const STORAGE_PRICE_PER_BYTE: u128 = 10_000_000_000_000_000_000; // 1e19yN, 0.00001N

/// Internal args format for meta call.
#[derive(Debug)]
pub struct InternalMetaCallArgs {
    pub sender: Address,
    pub nonce: U256,
    pub fee_amount: U256,
    pub fee_address: Address,
    pub contract_address: Address,
    pub value: U256,
    pub input: Vec<u8>,
}

#[allow(dead_code)]
pub fn u256_to_arr(value: &U256) -> [u8; 32] {
    let mut result = [0u8; 32];
    value.to_big_endian(&mut result);
    result
}

const HEX_ALPHABET: &[u8; 16] = b"0123456789abcdef";

#[allow(dead_code)]
pub fn bytes_to_hex(v: &[u8]) -> String {
    let mut result = String::new();
    for x in v {
        result.push(HEX_ALPHABET[(x / 16) as usize] as char);
        result.push(HEX_ALPHABET[(x % 16) as usize] as char);
    }
    result
}

#[cfg(feature = "contract")]
#[inline]
pub fn keccak(data: &[u8]) -> H256 {
    sdk::keccak(data)
}

#[cfg(not(feature = "contract"))]
#[inline]
pub fn keccak(data: &[u8]) -> H256 {
    H256::from_slice(Keccak256::digest(data).as_slice())
}

#[allow(dead_code)]
pub fn near_account_to_evm_address(addr: &[u8]) -> Address {
    Address::from_slice(&keccak(addr)[12..])
}

pub struct Stack<T> {
    stack: Vec<T>,
    boundaries: Vec<usize>,
}

impl<T> Stack<T> {
    pub fn new() -> Self {
        let mut boundaries = Vec::new();
        boundaries.push(0);
        Self {
            stack: Vec::new(),
            boundaries,
        }
    }

    pub fn enter(&mut self) {
        self.boundaries.push(self.stack.len());
    }

    pub fn commit(&mut self) {
        self.boundaries.pop().unwrap();
    }

    pub fn discard(&mut self) {
        let boundary = self.boundaries.pop().unwrap();
        while self.stack.len() > boundary {
            self.stack.pop().unwrap();
        }
    }

    pub fn push(&mut self, value: T) {
        self.stack.push(value);
    }

    pub fn into_vec(self) -> Vec<T> {
        self.stack
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex() {
        assert_eq!(
            bytes_to_hex(&[0u8, 1u8, 255u8, 16u8]),
            "0001ff10".to_string()
        );
    }

    /// Build view of the stack. Intervals between None values are scopes.
    fn view_stack(stack: &Stack<i32>) -> Vec<Option<i32>> {
        let mut res = vec![];
        let mut pnt = 0;

        for &pos in stack.boundaries.iter() {
            while pnt < pos {
                res.push(Some(stack.stack[pnt]));
                pnt += 1;
            }
            res.push(None);
        }

        while pnt < stack.stack.len() {
            res.push(Some(stack.stack[pnt]));
            pnt += 1;
        }

        res
    }

    fn check_stack(stack: &Stack<i32>, expected: Vec<Option<i32>>) {
        if let Some(&last) = stack.boundaries.last() {
            assert!(last <= stack.stack.len());
        }
        assert_eq!(view_stack(stack), expected);
    }

    #[test]
    fn test_stack() {
        let mut stack = Stack::new();

        stack.push(1); // [ $, 1]
        check_stack(&stack, vec![None, Some(1)]);
        stack.push(2); // [ $, 1, 2 ]
        check_stack(&stack, vec![None, Some(1), Some(2)]);
        stack.enter(); // [$, 1, 2, $]
        check_stack(&stack, vec![None, Some(1), Some(2), None]);
        stack.push(3); // [$, 1, 2, $, 3]
        check_stack(&stack, vec![None, Some(1), Some(2), None, Some(3)]);
        stack.discard(); // [$, 1, 2]
        check_stack(&stack, vec![None, Some(1), Some(2)]);
        stack.enter();
        check_stack(&stack, vec![None, Some(1), Some(2), None]);
        stack.push(4); // [$, 1, 2, $, 4]
        check_stack(&stack, vec![None, Some(1), Some(2), None, Some(4)]);
        stack.enter(); // [$, 1, 2, $, 4, $]
        check_stack(&stack, vec![None, Some(1), Some(2), None, Some(4), None]);
        stack.push(5); // [$, 1, 2, $, 4, $, 5]
        check_stack(
            &stack,
            vec![None, Some(1), Some(2), None, Some(4), None, Some(5)],
        );
        stack.commit(); // [$, 1, 2, $, 4, 5]
        check_stack(&stack, vec![None, Some(1), Some(2), None, Some(4), Some(5)]);
        stack.discard(); // [$, 1, 2]
        check_stack(&stack, vec![None, Some(1), Some(2)]);
        stack.push(6); // [$, 1, 2, 6]
        check_stack(&stack, vec![None, Some(1), Some(2), Some(6)]);
        stack.enter(); // [$, 1, 2, 6, $]
        check_stack(&stack, vec![None, Some(1), Some(2), Some(6), None]);
        stack.enter(); // [$, 1, 2, 6, $, $]
        check_stack(&stack, vec![None, Some(1), Some(2), Some(6), None, None]);
        stack.enter(); // [$, 1, 2, 6, $, $, $]
        check_stack(
            &stack,
            vec![None, Some(1), Some(2), Some(6), None, None, None],
        );
        stack.commit(); // [$, 1, 2, 6, $, $]
        check_stack(&stack, vec![None, Some(1), Some(2), Some(6), None, None]);
        stack.discard(); // [$, 1, 2, 6, $]
        check_stack(&stack, vec![None, Some(1), Some(2), Some(6), None]);
        stack.push(7);

        assert_eq!(stack.into_vec(), vec![1, 2, 6, 7]);
    }
}

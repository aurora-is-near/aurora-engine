use crate::prelude::*;

/// Pad the input with a given length, if necessary.
pub(super) fn pad_input(input: &[u8], len: usize) -> Vec<u8> {
    let input = if input.len() < len {
        let mut input = input.to_vec();
        input.reserve_exact(len);
        for _ in 0..(128 - input.len()) {
            input.push(0);
        }
        input
    } else {
        input.to_vec()
    };

    input
}

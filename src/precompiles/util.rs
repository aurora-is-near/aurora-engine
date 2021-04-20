use crate::prelude::*;

/// Pad the input with a given length, if necessary.
pub(super) fn pad_input(input: &[u8], len: usize) -> Vec<u8> {
    let mut input = input.to_vec();
    input.resize(len, 0);

    input
}

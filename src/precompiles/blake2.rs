use crate::prelude::Vec;

/// See: https://eips.ethereum.org/EIPS/eip-152
/// See: https://etherscan.io/address/0000000000000000000000000000000000000009
/// NOTE: Shouldn't there be gas checks here?
pub(crate) fn blake2f(input: &[u8]) -> Vec<u8> {
    let mut rounds_bytes = [0u8; 4];
    rounds_bytes.copy_from_slice(&input[0..4]);
    let rounds = u32::from_be_bytes(rounds_bytes);

    let mut h = [0u64; 8];
    for (mut x, value) in h.iter_mut().enumerate() {
        let mut word: [u8; 8] = [0u8; 8];
        x = x * 8 + 4;
        word.copy_from_slice(&input[x..(x + 8)]);
        *value = u64::from_be_bytes(word);
    }

    let mut m = [0u64; 16];
    for (mut x, value) in m.iter_mut().enumerate() {
        let mut word: [u8; 8] = [0u8; 8];
        x = x * 8 + 68;
        word.copy_from_slice(&input[x..(x + 8)]);
        *value = u64::from_be_bytes(word);
    }

    let mut t: [u64; 2] = [0u64; 2];
    for (mut x, value) in t.iter_mut().enumerate() {
        let mut word: [u8; 8] = [0u8; 8];
        x = x * 8 + 196;
        word.copy_from_slice(&input[x..(x + 8)]);
        *value = u64::from_be_bytes(word);
    }

    let finished = input[212] != 0;

    let res = &*blake2::blake2b_f(rounds, h, m, t, finished);
    let mut l = [0u8; 32];
    let mut h = [0u8; 32];
    l.copy_from_slice(&res[..32]);
    h.copy_from_slice(&res[32..64]);

    let mut res = l.to_vec();
    res.extend_from_slice(&h.to_vec());
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blake2f() {
        let mut v = [0u8; 213];
        let rounds: [u8; 4] = 12u32.to_be_bytes();
        v[..4].copy_from_slice(&rounds);
        let h: [u64; 8] = [
            0x6a09e667f2bdc948,
            0xbb67ae8584caa73b,
            0x3c6ef372fe94f82b,
            0xa54ff53a5f1d36f1,
            0x510e527fade682d1,
            0x9b05688c2b3e6c1f,
            0x1f83d9abfb41bd6b,
            0x5be0cd19137e2179,
        ];
        for (mut x, value) in h.iter().enumerate() {
            let value: [u8; 8] = value.to_be_bytes();
            x = x * 8 + 4;

            v[x..(x + 8)].copy_from_slice(&value);
        }

        let m: [u64; 16] = [
            0x0000000000636261,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
            0x0000000000000000,
        ];
        for (mut x, value) in m.iter().enumerate() {
            let value: [u8; 8] = value.to_be_bytes();
            x = x * 8 + 68;
            v[x..(x + 8)].copy_from_slice(&value);
        }

        let t: [u64; 2] = [3, 0];
        for (mut x, value) in t.iter().enumerate() {
            let value: [u8; 8] = value.to_be_bytes();
            x = x * 8 + 196;
            v[x..(x + 8)].copy_from_slice(&value);
        }

        let bool = 1;
        v[212] = bool;

        let expected = &*hex::decode(
            "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d1\
                7d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923",
        )
        .unwrap();
        let res = blake2f(&v);
        assert_eq!(res, expected);
    }
}

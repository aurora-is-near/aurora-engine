use crate::prelude::{Address, Borrowed, Vec, H160, H256, U256};
use evm::{Context, ExitError, ExitSucceed};
use num_bigint::BigUint;

type PrecompileResult = Result<(ExitSucceed, Vec<u8>, u64), ExitError>;

#[allow(dead_code)]
pub fn no_precompiles(
    _address: Address,
    _input: &[u8],
    _target_gas: Option<u64>,
    _context: &Context,
) -> Option<PrecompileResult> {
    None // no precompiles supported
}

#[allow(dead_code)]
pub fn istanbul_precompiles(
    address: Address,
    input: &[u8],
    target_gas: Option<u64>,
    _context: &Context,
) -> Option<PrecompileResult> {
    match address.to_low_u64_be() {
        1 => Some(Ok((
            ExitSucceed::Returned,
            ecrecover_raw(input).as_bytes().to_vec(),
            0,
        ))),
        2 => Some(Ok((
            ExitSucceed::Returned,
            sha256(input).as_bytes().to_vec(),
            0,
        ))),
        3 => Some(Ok((
            ExitSucceed::Returned,
            ripemd160(input).as_bytes().to_vec(),
            0,
        ))),
        4 => Some(Ok((ExitSucceed::Returned, identity(input).to_vec(), 0))),
        5 => match modexp(input, target_gas) {
            Ok(r) => Some(Ok((ExitSucceed::Returned, r, 0))),
            Err(e) => Some(Err(e)),
        },
        6 => todo!(), // TODO: implement alt_bn128_add()
        7 => todo!(), // TODO: implement alt_bn128_mul()
        8 => todo!(), // TODO: implement alt_bn128_pair()
        9 => Some(Ok((ExitSucceed::Returned, blake2f(input), 0))),
        // Not supported.
        _ => None,
    }
}

fn ecrecover_raw(input: &[u8]) -> Address {
    assert_eq!(input.len(), 128); // input is (hash, v, r, s), each typed as a uint256

    let mut hash = [0; 32];
    hash.copy_from_slice(&input[0..32]);

    let mut signature = [0; 65]; // signature is (r, s, v), typed (uint256, uint256, uint8)
    signature[0..32].copy_from_slice(&input[64..]); // r
    signature[32..64].copy_from_slice(&input[96..]); // s
    signature[64] = input[63]; // v

    ecrecover(H256::from_slice(&hash), &signature).unwrap_or_else(|_| Address::zero())
}

#[allow(dead_code)]
pub(crate) fn ecverify(hash: H256, signature: &[u8], signer: Address) -> bool {
    matches!(ecrecover(hash, signature), Ok(s) if s == signer)
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
/// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
/// See: https://etherscan.io/address/0000000000000000000000000000000000000001
pub(crate) fn ecrecover(hash: H256, signature: &[u8]) -> Result<Address, ExitError> {
    use sha3::Digest;
    assert_eq!(signature.len(), 65);

    let hash = secp256k1::Message::parse_slice(hash.as_bytes()).unwrap();
    let v = signature[64];
    let signature = secp256k1::Signature::parse_slice(&signature[0..64]).unwrap();
    let bit = match v {
        0..=26 => v,
        _ => v - 27,
    };

    if let Ok(recovery_id) = secp256k1::RecoveryId::parse(bit) {
        if let Ok(public_key) = secp256k1::recover(&hash, &signature, &recovery_id) {
            // recover returns a 65-byte key, but addresses come from the raw 64-byte key
            let r = sha3::Keccak256::digest(&public_key.serialize()[1..]);
            return Ok(Address::from_slice(&r[12..]));
        }
    }
    Err(ExitError::Other(Borrowed("invalid ECDSA signature")))
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
/// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
/// See: https://etherscan.io/address/0000000000000000000000000000000000000002
#[cfg(not(feature = "contract"))]
fn sha256(input: &[u8]) -> H256 {
    use sha2::Digest;
    let hash = sha2::Sha256::digest(input);
    H256::from_slice(&hash)
}
#[cfg(feature = "contract")]
fn sha256(input: &[u8]) -> H256 {
    use crate::sdk;
    sdk::sha256(input)
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
/// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
/// See: https://etherscan.io/address/0000000000000000000000000000000000000003
fn ripemd160(input: &[u8]) -> H160 {
    use ripemd160::Digest;
    let hash = ripemd160::Ripemd160::digest(input);
    H160::from_slice(&hash)
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
/// See: https://etherscan.io/address/0000000000000000000000000000000000000004
fn identity(input: &[u8]) -> &[u8] {
    input
}

/// See: https://eips.ethereum.org/EIPS/eip-198
/// See: https://etherscan.io/address/0000000000000000000000000000000000000005
fn modexp(input: &[u8], target_gas: Option<u64>) -> Result<Vec<u8>, ExitError> {
    fn adj_exp_len(exp_len: U256, base_len: U256, bytes: &[u8]) -> U256 {
        let mut exp32_bytes = Vec::with_capacity(32);
        for i in 0..32 {
            if U256::from(96) + base_len + U256::from(1) >= U256::from(bytes.len()) {
                exp32_bytes.push(0u8);
            } else {
                let base_len_i = base_len.as_usize();
                let bytes_i = 96 + base_len_i + i;
                if let Some(byte) = bytes.get(bytes_i) {
                    exp32_bytes.push(*byte);
                } else {
                    // Pad out the data if the byte is empty.
                    exp32_bytes.push(0u8);
                }
            }
        }
        let exp32 = U256::from(exp32_bytes.as_slice());

        if exp_len <= U256::from(32) && exp32 == U256::zero() {
            U256::zero()
        } else if exp_len <= U256::from(32) {
            U256::from(exp32.bits())
        } else {
            // else > 32
            U256::from(8) * (exp_len - U256::from(32)) + U256::from(exp32.bits())
        }
    }

    fn mult_complexity(x: U256) -> Result<U256, ExitError> {
        if x <= U256::from(64) {
            Ok(x * x)
        } else if x <= U256::from(1_024) {
            Ok(x * x / U256::from(4) + U256::from(96) * x - U256::from(3_072))
        } else {
            let (sqroot, overflow) = x.overflowing_mul(x);
            if overflow {
                Err(ExitError::OutOfGas)
            } else {
                Ok(sqroot / U256::from(16) + U256::from(480) * x - U256::from(199_680))
            }
        }
    }

    let base_len = U256::from(&input[0..32]);
    let exp_len = U256::from(&input[32..64]);
    let mod_len = U256::from(&input[64..96]);

    let mul = mult_complexity(core::cmp::max(mod_len, base_len))?;
    let adj =
        core::cmp::max(adj_exp_len(exp_len, base_len, &input), U256::from(1)) / U256::from(20);
    let (gas_val, overflow) = mul.overflowing_mul(adj);
    if overflow {
        return Err(ExitError::OutOfGas);
    }

    // If we have a target gas, check if we go over.
    if let Some(target_gas) = target_gas {
        let gas = gas_val.as_u64();
        if gas > target_gas {
            return Err(ExitError::OutOfGas);
        }
    }

    let base_len = base_len.as_usize();
    let mut base_bytes = Vec::with_capacity(32);
    for i in 0..base_len {
        if 96 + i >= input.len() {
            base_bytes.push(0u8);
        } else {
            base_bytes.push(input[96 + i]);
        }
    }

    let exp_len = exp_len.as_usize();
    let mut exp_bytes = Vec::with_capacity(32);
    for i in 0..exp_len {
        if 96 + base_len + i >= input.len() {
            exp_bytes.push(0u8);
        } else {
            exp_bytes.push(input[96 + base_len + i]);
        }
    }

    let mod_len = mod_len.as_usize();
    let mut mod_bytes = Vec::with_capacity(32);
    for i in 0..mod_len {
        if 96 + base_len + exp_len + i >= input.len() {
            mod_bytes.push(0u8);
        } else {
            mod_bytes.push(input[96 + base_len + exp_len + i]);
        }
    }

    let base = BigUint::from_bytes_be(&base_bytes);
    let exponent = BigUint::from_bytes_be(&exp_bytes);
    let modulus = BigUint::from_bytes_be(&mod_bytes);

    Ok(base.modpow(&exponent, &modulus).to_bytes_be())
}

/// See: https://eips.ethereum.org/EIPS/eip-196
/// See: https://etherscan.io/address/0000000000000000000000000000000000000006
#[allow(dead_code)]
fn alt_bn128_add(_ax: U256, _ay: U256, _bx: U256, _by: U256) {
    // TODO: implement alt_bn128_add
}

/// See: https://eips.ethereum.org/EIPS/eip-196
/// See: https://etherscan.io/address/0000000000000000000000000000000000000007
#[allow(dead_code)]
fn alt_bn128_mul(_x: U256, _y: U256, _scalar: U256) {
    // TODO: implement alt_bn128_mul
}

/// See: https://eips.ethereum.org/EIPS/eip-197
/// See: https://etherscan.io/address/0000000000000000000000000000000000000008
#[allow(dead_code)]
fn alt_bn128_pair(_input: Vec<u8>) -> U256 {
    U256::zero() // TODO: implement alt_bn128_pairing
}

/// See: https://eips.ethereum.org/EIPS/eip-152
/// See: https://etherscan.io/address/0000000000000000000000000000000000000009
/// NOTE: Shouldn't there be gas checks here?
fn blake2f(input: &[u8]) -> Vec<u8> {
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
    fn test_ecverify() {
        let hash = H256::from_slice(
            &hex::decode("1111111111111111111111111111111111111111111111111111111111111111")
                .unwrap(),
        );
        let signature =
                &hex::decode("b9f0bb08640d3c1c00761cdd0121209268f6fd3816bc98b9e6f3cc77bf82b69812ac7a61788a0fdc0e19180f14c945a8e1088a27d92a74dce81c0981fb6447441b")
                    .unwrap();
        let signer =
            Address::from_slice(&hex::decode("1563915e194D8CfBA1943570603F7606A3115508").unwrap());
        assert!(ecverify(hash, &signature, signer));
    }

    #[test]
    fn test_sha256() {
        assert_eq!(
            sha256(b""),
            H256::from_slice(
                &hex::decode("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_ripemd160() {
        assert_eq!(
            ripemd160(b""),
            H160::from_slice(&hex::decode("9c1185a5c5e9fc54612808977ee8f548b2258d31").unwrap())
        );
    }

    #[test]
    fn test_modexp() {
        let test_input1 = hex::decode(
            "\
            0000000000000000000000000000000000000000000000000000000000000001\
            0000000000000000000000000000000000000000000000000000000000000020\
            0000000000000000000000000000000000000000000000000000000000000020\
            03\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f",
        )
        .unwrap();
        let res = U256::from_big_endian(&modexp(&test_input1, None).unwrap());
        assert_eq!(res, U256::from(1));

        let test_input2 = hex::decode(
            "0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000020\
            0000000000000000000000000000000000000000000000000000000000000020\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f",
        )
        .unwrap();
        let res = U256::from_big_endian(&modexp(&test_input2, None).unwrap());
        assert_eq!(res, U256::from(0));

        let test_input3 = hex::decode(
            "0000000000000000000000000000000000000000000000000000000000000000\
            0000000000000000000000000000000000000000000000000000000000000020\
            ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd",
        )
        .unwrap();
        assert!(modexp(&test_input3, None).is_err());

        let test_input4 = hex::decode(
            "0000000000000000000000000000000000000000000000000000000000000001\
            0000000000000000000000000000000000000000000000000000000000000002\
            0000000000000000000000000000000000000000000000000000000000000020\
            03\
            ffff\
            8000000000000000000000000000000000000000000000000000000000000000\
            07",
        )
        .unwrap();
        let expected = U256::from_big_endian(
            &hex::decode("3b01b01ac41f2d6e917c6d6a221ce793802469026d9ab7578fa2e79e4da6aaab")
                .unwrap(),
        );
        let res = U256::from_big_endian(&modexp(&test_input4, None).unwrap());
        assert_eq!(res, expected);

        let test_input5 = hex::decode(
            "0000000000000000000000000000000000000000000000000000000000000001\
            0000000000000000000000000000000000000000000000000000000000000002\
            0000000000000000000000000000000000000000000000000000000000000020\
            03\
            ffff\
            80",
        )
        .unwrap();
        let expected = U256::from_big_endian(
            &hex::decode("3b01b01ac41f2d6e917c6d6a221ce793802469026d9ab7578fa2e79e4da6aaab")
                .unwrap(),
        );
        let res = U256::from_big_endian(&modexp(&test_input5, None).unwrap());
        assert_eq!(res, expected);
    }

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

    #[test]
    fn test_identity() {
        assert_eq!(identity(b""), b"")
    }
}

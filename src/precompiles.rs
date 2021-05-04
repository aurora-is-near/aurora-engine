use crate::parameters::BridgedTokenWithdrawEthConnectorArgs;
use crate::prelude::{Address, Borrowed, Vec, H160, H256, U256};
use evm::{Context, ExitError, ExitSucceed};

use alloc::string::{String, ToString};
use borsh::BorshSerialize;

use crate::parameters::{BridgedTokenWithdrawArgs, NEP41TransferCallArgs};
use crate::sdk;
use crate::types::{AccountId, Gas};

type PrecompileResult = Result<(ExitSucceed, Vec<u8>, u64), ExitError>;

// Computed as: near_account_to_evm_address("exitToEthereum".as_bytes()).to_low_u64_be()
const EXIT_TO_ETHEREUM_ID: u64 = 17176159495920586411;

// Computed as: near_account_to_evm_address("exitToNear".as_bytes()).to_low_u64_be()
const EXIT_TO_NEAR_ID: u64 = 11421322804619973199;

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
    _target_gas: Option<u64>,
    context: &Context,
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
        5 => todo!(), // TODO: implement modexp()
        6 => todo!(), // TODO: implement alt_bn128_add()
        7 => todo!(), // TODO: implement alt_bn128_mul()
        8 => todo!(), // TODO: implement alt_bn128_pair()
        9 => todo!(), // TODO: implement blake2f()
        EXIT_TO_NEAR_ID => {
            exit_to_near(&input, &context);
            Some(Ok((ExitSucceed::Returned, Vec::new(), 0)))
        }
        EXIT_TO_ETHEREUM_ID => {
            exit_to_ethereum(&input, &context);
            Some(Ok((ExitSucceed::Returned, Vec::new(), 0)))
        }
        // Not supported.
        _ => None,
    }
}

#[allow(dead_code)]
fn exit_to_near(input: &[u8], context: &Context) {
    // TODO: Determine the correct amount of gas
    const GAS_FOR_FT_TRANSFER: Gas = 50_000;
    // if Self::required_gas(input)? > target_gas {
    //     return Err(ExitError::OutOfGas);
    // }

    let (nep141account, args) = if context.apparent_value != U256::from(0) {
        // ETH transfer
        //
        // Input slice format:
        //      recipient_account_id (bytes) - the NEAR recipient account which will receive NEP-141 ETH tokens

        (
            String::from_utf8(sdk::current_account_id()).unwrap(),
            NEP41TransferCallArgs {
                receiver_id: String::from_utf8(input.to_vec()).unwrap(),
                amount: context.apparent_value.as_u128(),
                memo: None,
            },
        )
    } else {
        // ERC20 transfer
        //
        // This precompile branch is expected to be called from the ERC20 burn function\
        //
        // Input slice format:
        //      sender (20 bytes) - Eth address of the address that burned his erc20 tokens
        //      amount (U256 le bytes) - the amount that was burned
        //      recipient_account_id (bytes) - the NEAR recipient account which will receive NEP-141 tokens

        //TODO: add method in Aurora connector and call promise `get_near_account_for_evm_token(context.caller)`
        let nep141address = context.caller.to_string();

        let mut input = input;

        let mut sender = [0u8; 20];
        sender.copy_from_slice(&input[..20]);
        input = &input[20..];

        let amount = U256::from_little_endian(&input[..32]).as_u128();
        input = &input[32..];

        let receiver_account_id: AccountId = String::from_utf8(input.to_vec()).unwrap();

        (
            nep141address,
            NEP41TransferCallArgs {
                receiver_id: receiver_account_id,
                amount,
                memo: None,
            },
        )
    };

    let promise0 = sdk::promise_create(
        nep141account,
        b"ft_transfer",
        BorshSerialize::try_to_vec(&args).ok().unwrap().as_slice(),
        0,
        GAS_FOR_FT_TRANSFER,
    );

    sdk::promise_return(promise0);
}

#[allow(dead_code)]
fn exit_to_ethereum(input: &[u8], context: &Context) {
    // TODO: Determine the correct amount of gas
    const GAS_FOR_WITHDRAW: Gas = 50_000;
    // if Self::required_gas(input)? > target_gas {
    //     return Err(ExitError::OutOfGas);
    // }

    let (nep141account, serialized_args) = if context.apparent_value != U256::from(0) {
        // ETH transfer
        //
        // Input slice format:
        //      eth_recipient (20 bytes) - the address of recipient which will receive ETH on Ethereum

        let eth_recipient: AccountId = String::from_utf8(input.to_vec()).unwrap();
        let args = BridgedTokenWithdrawEthConnectorArgs {
            amount: context.apparent_value.as_u128(),
            recipient: eth_recipient,
        };

        (
            String::from_utf8(sdk::current_account_id()).unwrap(),
            BorshSerialize::try_to_vec(&args).ok().unwrap(),
        )
    } else {
        // ERC-20 transfer
        //
        // This precompile branch is expected to be called from the ERC20 withdraw function
        // (or burn function with some flag provided that this is expected to be withdrawn)
        //
        // Input slice format:
        //      sender (20 bytes) - Eth address of the address that burned his erc20 tokens
        //      amount (U256 le bytes) - the amount that was burned
        //      eth_recipient (20 bytes) - the address of recipient which will receive ETH on Ethereum

        //TODO: add method in Aurora connector and call promise `get_near_account_for_evm_token(context.caller)`
        let nep141address = context.caller.to_string();

        let mut input = input;

        let mut sender = [0u8; 20];
        sender.copy_from_slice(&input[..20]);
        input = &input[20..];

        let amount = U256::from_little_endian(&input[..32]).as_u128();
        input = &input[32..];

        let eth_recipient: AccountId = String::from_utf8(input.to_vec()).unwrap();
        let args = BridgedTokenWithdrawArgs {
            recipient: eth_recipient,
            amount,
        };

        (
            nep141address,
            BorshSerialize::try_to_vec(&args).ok().unwrap(),
        )
    };

    let promise0 = sdk::promise_create(
        nep141account,
        b"withdraw",
        serialized_args.as_slice(),
        0,
        GAS_FOR_WITHDRAW,
    );

    sdk::promise_return(promise0);
}

#[allow(dead_code)]
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
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000001
#[allow(dead_code)]
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
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000002
#[cfg(not(feature = "contract"))]
fn sha256(input: &[u8]) -> H256 {
    use sha2::Digest;
    let hash = sha2::Sha256::digest(input);
    H256::from_slice(&hash)
}
#[cfg(feature = "contract")]
fn sha256(input: &[u8]) -> H256 {
    sdk::sha256(input)
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
/// See: https://docs.soliditylang.org/en/develop/units-and-global-variables.html#mathematical-and-cryptographic-functions
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000003
fn ripemd160(input: &[u8]) -> H160 {
    use ripemd160::Digest;
    let hash = ripemd160::Ripemd160::digest(input);
    H160::from_slice(&hash)
}

/// See: https://ethereum.github.io/yellowpaper/paper.pdf
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000004
fn identity(input: &[u8]) -> &[u8] {
    input
}

/// See: https://eips.ethereum.org/EIPS/eip-198
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000005
#[allow(dead_code)]
fn modexp(_base: U256, _exponent: U256, _modulus: U256) -> U256 {
    U256::zero() // TODO: implement MODEXP
}

/// See: https://eips.ethereum.org/EIPS/eip-196
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000006
#[allow(dead_code)]
fn alt_bn128_add(_ax: U256, _ay: U256, _bx: U256, _by: U256) {
    // TODO: implement alt_bn128_add
}

/// See: https://eips.ethereum.org/EIPS/eip-196
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000007
#[allow(dead_code)]
fn alt_bn128_mul(_x: U256, _y: U256, _scalar: U256) {
    // TODO: implement alt_bn128_mul
}

/// See: https://eips.ethereum.org/EIPS/eip-197
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000008
#[allow(dead_code)]
fn alt_bn128_pair(_input: Vec<u8>) -> U256 {
    U256::zero() // TODO: implement alt_bn128_pairing
}

/// See: https://eips.ethereum.org/EIPS/eip-152
/// See: https://etherscan.io/address/0x0000000000000000000000000000000000000009
#[allow(dead_code)]
fn blake2f(_rounds: u32, _h: [U256; 2], _m: [U256; 4], _t: [u64; 2], _f: bool) -> [U256; 2] {
    [U256::zero(), U256::zero()] // TODO: implement BLAKE2f
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::near_account_to_evm_address;

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
    fn test_identity() {
        assert_eq!(identity(b""), b"")
    }

    #[test]
    fn test_precompile_id() {
        assert_eq!(
            EXIT_TO_ETHEREUM_ID,
            near_account_to_evm_address("exitToEthereum".as_bytes()).to_low_u64_be()
        );
        assert_eq!(
            EXIT_TO_NEAR_ID,
            near_account_to_evm_address("exitToNear".as_bytes()).to_low_u64_be()
        );
    }
}

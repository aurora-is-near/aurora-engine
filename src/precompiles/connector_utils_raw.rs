use crate::precompiles::{Precompile, PrecompileResult};
use crate::prelude::{mem, Borrowed, TryInto};
use evm::{Context, ExitError, ExitSucceed};

pub struct TransferEthToNear;

impl Precompile for TransferEthToNear {
    fn required_gas(input: &[u8]) -> Result<u64, ExitError> {
        //TODO
        Ok(0)
    }

    /// Input slice format: [
    /// token_account_id_len: (1 byte)
    /// token_account_id: (`account_id_len` le bytes)
    /// receiver_account_id_len: (1 byte)
    /// receiver_account_id: (`account_id_len` le bytes)
    /// amount (U256 le bytes)
    /// memo_len (optional u32 le bytes)
    /// memo (optional `memo_len` le bytes)
    /// ]
    ///
    /// address: 0x0000000000000000000000000000000000000ff0
    #[cfg(feature = "contract")]
    // ft_transfer_raw
    fn ft_transfer_raw(input: &[u8], target_gas: u64, context: &Context) -> PrecompileResult {
        use alloc::string::String;
        use borsh::BorshSerialize;
        use primitive_types::U256;

        use crate::sdk;
        use crate::types::Gas;
        use crate::parameters::TransferCallArgs;

        const U32_SIZE: usize = 32 / 8;
        const U256_SIZE: usize = 256 / 8;

        const GAS_FOR_FT_TRANSFER: Gas = 50_000;

        if Self::required_gas(input)? > target_gas {
            return Err(ExitError::OutOfGas);
        }

        let mut cursor = 0;

        let mut token_account_id_len_bytes = [0u8; U32_SIZE];
        token_account_id_len_bytes[0] = input[cursor];
        cursor += 1;

        let token_account_id_len = u32::from_le_bytes(token_account_id_len_bytes) as usize;
        let token_account_id: AccountId = String::from_utf8((&input[cursor..cursor + token_account_id_len]).to_vec()).unwrap();
        cursor += token_account_id_len;

        let mut receiver_account_id_len_bytes = [0u8; U32_SIZE];
        receiver_account_id_len_bytes[0] = input[cursor];
        cursor += 1;

        let receiver_account_id_len = u32::from_le_bytes(receiver_account_id_len_bytes) as usize;
        let receiver_account_id: AccountId = String::from_utf8((&input[cursor..cursor + receiver_account_id_len]).to_vec()).unwrap();
        cursor += receiver_account_id_len;

        let mut amount_bytes = [0u8; U256_SIZE];
        amount_bytes.copy_from_slice(&input[cursor..cursor + U256_SIZE]);
        let amount = U256::from_little_endian(&amount_bytes).as_u128();
        cursor += U256_SIZE;

        let mut memo: Option<String> = if cursor < (input.len() - 1) {
            let mut memo_len_bytes = [0u8; U32_SIZE];
            memo_len_bytes.copy_from_slice(&input[cursor..cursor + U32_SIZE]);
            let memo_len = u32::from_le_bytes(memo_len_bytes) as usize;
            cursor += U32_SIZE;

            let memo_string = String::from_utf8((&input[cursor..cursor + memo_len]).to_vec()).unwrap();
            cursor += memo_len;

            Some(memo_string)
        } else {
            None
        };

        let args = TransferCallArgs {
            receiver_id: receiver_account_id,
            amount: amount,
            memo: memo,
        };

        let promise0 = sdk::promise_create(
            &sdk::current_account_id(),
            b"ft_transfer",
            BorshSerialize::try_to_vec(&args).ok().unwrap().as_slice(),
            0,
            GAS_FOR_FT_TRANSFER,
        );
        sdk::promise_return(promise0);

        Ok((ExitSucceed::Returned, vec![], 0))
    }
}


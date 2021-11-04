use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{U128, ValidAccountId};
use near_sdk::{near_bindgen, log, PromiseOrValue};

/// Will happily take and NEP-141
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Default)]
struct DummyFungibleTokenReceiver;

#[near_bindgen]
impl FungibleTokenReceiver for DummyFungibleTokenReceiver {
    fn ft_on_transfer(
        &mut self,
        sender_id: ValidAccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        log!("in {} tokens from @{} ft_on_transfer, msg = {}", amount.0, sender_id.as_ref(), msg);
        PromiseOrValue::Value(U128::from(0))
    }
}

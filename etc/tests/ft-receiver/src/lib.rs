use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::json_types::U128;
use near_sdk::{log, near, AccountId, PromiseOrValue};

/// Will happily take and NEP-141
#[near(contract_state)]
#[derive(Default)]
struct DummyFungibleTokenReceiver;

#[near]
impl FungibleTokenReceiver for DummyFungibleTokenReceiver {
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        log!(
            "in {} tokens from @{} ft_on_transfer, msg = {}",
            amount.0,
            sender_id,
            msg
        );
        PromiseOrValue::Value(U128::from(0))
    }
}

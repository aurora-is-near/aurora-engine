use near_sdk::{near, AccountId};
use near_token::NearToken;

#[near(contract_state)]
#[derive(Default)]
pub struct MockController;

#[near]
impl MockController {
    pub fn finish_withdraw_v2(
        &mut self,
        #[serializer(borsh)] sender_id: AccountId,
        #[serializer(borsh)] amount: NearToken,
        #[serializer(borsh)] recipient: String,
    ) {
        near_sdk::log!(
            "finish_withdraw_v2 called from: {sender_id} amount: {amount} recipient: {recipient}"
        );
    }
}

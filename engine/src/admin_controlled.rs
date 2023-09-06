use aurora_engine_types::account_id::AccountId;
use aurora_engine_types::parameters::connector::WithdrawSerializeType;

pub trait AdminControlled {
    fn get_eth_connector_contract_account(&self) -> AccountId;
    fn set_eth_connector_contract_account(&mut self, account: &AccountId);
    fn get_withdraw_serialize_type(&self) -> WithdrawSerializeType;
    fn set_withdraw_serialize_type(&mut self, serialize_type: &WithdrawSerializeType);
}

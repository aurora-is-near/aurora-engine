use aurora_engine_types::account_id::AccountId;

pub trait AdminControlled {
    fn get_eth_connector_contract_account(&self) -> AccountId;
    fn set_eth_connector_contract_account(&mut self, account: AccountId);
}

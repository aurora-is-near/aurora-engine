use aurora_engine_types::account_id::AccountId;

pub type PausedMask = u8;

pub const ERR_PAUSED: &str = "ERR_PAUSED";

pub trait AdminControlled {
    /// Return the current mask representing all paused events.
    fn get_paused(&self) -> PausedMask;

    /// Update mask with all paused events.
    /// Implementor is responsible for guaranteeing that this function can only be
    /// called by owner of the contract.
    fn set_paused(&mut self, paused: PausedMask);

    /// Return if the contract is paused for the current flag and user
    fn is_paused(&self, flag: PausedMask, is_owner: bool) -> bool {
        (self.get_paused() & flag) != 0 && !is_owner
    }

    /// Asserts the passed paused flag is not set. Returns `PausedError` if paused.
    fn assert_not_paused(&self, flag: PausedMask, is_owner: bool) -> Result<(), PausedError> {
        if self.is_paused(flag, is_owner) {
            Err(PausedError)
        } else {
            Ok(())
        }
    }

    fn get_eth_connector_contract_account(&self) -> AccountId;

    fn set_eth_connector_contract_account(&mut self, account: AccountId);
}

pub struct PausedError;
impl AsRef<[u8]> for PausedError {
    fn as_ref(&self) -> &[u8] {
        ERR_PAUSED.as_bytes()
    }
}

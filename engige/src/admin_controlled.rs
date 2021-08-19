use crate::sdk;

pub type PausedMask = u8;

pub(crate) const ERR_PAUSED: &str = "ERR_PAUSED";

pub trait AdminControlled {
    /// Returns true if the current account is owner
    fn is_owner(&self) -> bool {
        sdk::current_account_id() == sdk::predecessor_account_id()
    }

    /// Return the current mask representing all paused events.
    fn get_paused(&self) -> PausedMask;

    /// Update mask with all paused events.
    /// Implementor is responsible for guaranteeing that this function can only be
    /// called by owner of the contract.
    fn set_paused(&mut self, paused: PausedMask);

    /// Return if the contract is paused for the current flag and user
    fn is_paused(&self, flag: PausedMask) -> bool {
        (self.get_paused() & flag) != 0 && !self.is_owner()
    }

    /// Asserts the passed paused flag is not set. Panics with "ERR_PAUSED" if the flag is set.
    fn assert_not_paused(&self, flag: PausedMask) {
        assert!(!self.is_paused(flag), "{}", ERR_PAUSED);
    }
}

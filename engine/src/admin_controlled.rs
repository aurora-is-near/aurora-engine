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

    /// Asserts the passed paused flag is not set. Panics with "ERR_PAUSED" if the flag is set.
    fn assert_not_paused(&self, flag: PausedMask, is_owner: bool) {
        assert!(!self.is_paused(flag, is_owner), "{}", ERR_PAUSED);
    }
}

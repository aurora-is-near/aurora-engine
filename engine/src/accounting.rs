use aurora_engine_types::U256;
use core::cmp::Ordering;

/// This struct tracks changes to the supply of a U256 quantity.
/// It is used in our code to keep track of the total supply of ETH on Aurora.
/// This struct is intentionally designed to avoid doing subtraction as much as possible
/// to avoid complexities of signed values and over/underflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Accounting {
    gained: U256,
    lost: U256,
}

impl Accounting {
    pub fn change(&mut self, amount: Change) {
        match amount.new_value.cmp(&amount.old_value) {
            Ordering::Greater => {
                let net_gained = amount.new_value - amount.old_value;
                self.gained = self.gained.saturating_add(net_gained);
            }
            Ordering::Less => {
                let net_lost = amount.old_value - amount.new_value;
                self.lost = self.lost.saturating_add(net_lost);
            }
            Ordering::Equal => (),
        }
    }

    pub fn remove(&mut self, amount: U256) {
        self.lost = self.lost.saturating_add(amount);
    }

    pub fn net(&self) -> Net {
        match self.gained.cmp(&self.lost) {
            Ordering::Equal => Net::Zero,
            Ordering::Greater => Net::Gained(self.gained - self.lost),
            Ordering::Less => Net::Lost(self.lost - self.gained),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Change {
    pub new_value: U256,
    pub old_value: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Net {
    Zero,
    Gained(U256),
    Lost(U256),
}

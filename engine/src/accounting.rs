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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_removing_loss_is_same_as_decreasing_change() {
        let loss = U256::from(16u64);
        let change = Change {
            new_value: U256::zero(),
            old_value: loss,
        };

        let mut changed_accounting = Accounting::default();
        changed_accounting.change(change);

        let mut removed_accounting = Accounting::default();
        removed_accounting.remove(loss);

        assert_eq!(removed_accounting, changed_accounting);
    }

    #[test]
    fn test_removing_loss_nets_loss() {
        let mut actual_accounting = Accounting::default();

        let loss = U256::from(16u64);

        actual_accounting.remove(loss);

        let actual_net = actual_accounting.net();
        let expected_net = Net::Lost(loss);

        assert_eq!(expected_net, actual_net);
    }

    #[test]
    fn test_equal_change_nets_zero() {
        let mut actual_accounting = Accounting::default();

        let value = U256::from(16u64);
        let equal_change = Change {
            new_value: value,
            old_value: value,
        };

        actual_accounting.change(equal_change);

        let actual_net = actual_accounting.net();
        let expected_net = Net::Zero;

        assert_eq!(expected_net, actual_net);
    }

    #[test]
    fn test_decreasing_change_nets_loss() {
        let mut actual_accounting = Accounting::default();

        let new_value = U256::from(16u64);
        let old_value = U256::from(32u64);
        let decreasing_change = Change {
            new_value,
            old_value,
        };

        actual_accounting.change(decreasing_change);

        let actual_net = actual_accounting.net();
        let expected_net = Net::Lost(U256::from(16u64));

        assert_eq!(expected_net, actual_net);
    }

    #[test]
    fn test_increasing_change_nets_gain() {
        let mut actual_accounting = Accounting::default();

        let new_value = U256::from(32u64);
        let old_value = U256::from(16u64);
        let increasing_change = Change {
            new_value,
            old_value,
        };

        actual_accounting.change(increasing_change);

        let actual_net = actual_accounting.net();
        let expected_net = Net::Gained(U256::from(16u64));

        assert_eq!(expected_net, actual_net);
    }
}

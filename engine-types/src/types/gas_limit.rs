use borsh::{BorshDeserialize, BorshSerialize};

/// The `gas_limit` field of `eth_call` is optional,
/// but a `gas_limit` is still required by Aurora Engine.
/// Therefore, the two options for a gas limit are either
/// `UserDefined` (when the `gas_limit` is given in the `eth_call` request)
/// or `Default` (when `gas_limit` is not given in the request).
/// A third option is given for the gas that the user provided a value
/// which is higher than Borealis Engine's computational limit for
/// `eth_call` (which is present to prevent DOS attacks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum GasLimit {
    UserDefined(u64),
    Default(u64),
    Limited { user_value: u64, limit: u64 },
}

impl GasLimit {
    #[inline]
    pub const fn value(self) -> u64 {
        match self {
            Self::UserDefined(value) => value,
            Self::Default(value) => value,
            Self::Limited { limit, .. } => limit,
        }
    }

    /// Get the user defined value (if any). This function
    /// ignores if an upper bound was imposed.
    pub const fn unlimited_user_defined_value(self) -> Option<u64> {
        match self {
            Self::Default(_) => None,
            Self::UserDefined(value) => Some(value),
            Self::Limited { user_value, .. } => Some(user_value),
        }
    }

    /// Impose an upper bound on the value of the gas limit.
    pub const fn limited(self, limit: u64) -> Self {
        match self {
            Self::UserDefined(user_value) if user_value > limit => {
                Self::Limited { user_value, limit }
            }
            Self::Default(value) if value > limit => Self::Default(limit),
            Self::Limited {
                user_value,
                limit: old_limit,
            } if old_limit > limit => Self::Limited { user_value, limit },
            limit_not_exceeded => limit_not_exceeded,
        }
    }
}

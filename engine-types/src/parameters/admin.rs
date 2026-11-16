//! JSON arguments of the owner-only `admin_call` contract method.
//!
//! `admin_call` makes the engine account send one batch of NEAR actions to another account.
//! The types in this module can express only the actions that method is allowed to send:
//! NEAR transfers and function calls. Every other [`PromiseAction`] (deploying code, managing
//! access keys, staking, creating or deleting accounts) has no representation here and
//! therefore fails to deserialize.
//!
//! Example input, where `e30=` is the base64 encoding of the JSON document `{}`:
//!
//! ```json
//! {
//!   "receiver_id": "token.near",
//!   "actions": [
//!     { "Transfer": { "amount": "1000000000000000000000000" } },
//!     {
//!       "FunctionCall": {
//!         "method_name": "storage_deposit",
//!         "args": "e30=",
//!         "deposit": "1250000000000000000000",
//!         "gas": 10000000000000
//!       }
//!     }
//!   ]
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::account_id::AccountId;
use crate::parameters::{PromiseAction, PromiseBatchAction};
use crate::types::{NearGas, Yocto};
use crate::{String, Vec};

/// JSON-encoded parameters for the `admin_call` function.
///
/// Describes one batch of actions sent from the engine account to `receiver_id`. NEAR executes
/// a batch in a single receipt: the actions run in the given order, and if any of them fails,
/// none of them takes effect.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AdminCallArgs {
    /// Account the actions are applied to: it receives the transferred NEAR, and its contract
    /// executes the function calls. The engine rejects its own account here.
    pub receiver_id: AccountId,
    /// Actions to perform, in order.
    pub actions: Vec<AdminCallAction>,
}

/// An action that `admin_call` may perform on behalf of the engine account.
///
/// This is deliberately a subset of [`PromiseAction`]. In JSON, an action is an object with a
/// single key, the variant name: `{"Transfer": {"amount": "1"}}`. Unknown fields inside a
/// variant are an error, so a misspelt optional field (`arg` instead of `args`) is rejected
/// rather than silently dropped.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub enum AdminCallAction {
    /// Transfers NEAR from the engine account's balance to the receiver.
    Transfer {
        /// Amount in `yoctoNEAR`, encoded as a decimal string.
        amount: Yocto,
    },
    /// Calls a method of the contract deployed on the receiver account.
    FunctionCall {
        /// Name of the method to call.
        method_name: String,
        /// Raw bytes passed to the method as its input, encoded as a base64 string (standard
        /// alphabet, with padding). If the method takes JSON arguments, this is the base64
        /// encoding of that JSON document. May be omitted to call the method with empty input.
        #[serde(default, skip_serializing_if = "Vec::is_empty", with = "base64_bytes")]
        args: Vec<u8>,
        /// Amount of `yoctoNEAR` attached to the call from the engine account's balance,
        /// encoded as a decimal string.
        deposit: Yocto,
        /// NEAR gas attached to the call, encoded as a JSON number. It is taken out of the gas
        /// prepaid for the `admin_call` transaction itself.
        gas: NearGas,
    },
}

/// Serde representation of a byte vector as a base64 string (standard alphabet, with padding).
mod base64_bytes {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::{String, Vec};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        // An owned `String` rather than `&str`: JSON may escape `/` as `\/`, and an escaped
        // string cannot be borrowed from the input.
        let encoded = String::deserialize(deserializer)?;
        STANDARD.decode(encoded).map_err(D::Error::custom)
    }
}

/// Converts the JSON arguments into the batch promise the engine schedules.
impl From<AdminCallArgs> for PromiseBatchAction {
    fn from(value: AdminCallArgs) -> Self {
        Self {
            target_account_id: value.receiver_id,
            actions: value.actions.into_iter().map(Into::into).collect(),
        }
    }
}

/// Maps an admin action onto the [`PromiseAction`] variant of the same name.
impl From<AdminCallAction> for PromiseAction {
    fn from(value: AdminCallAction) -> Self {
        match value {
            AdminCallAction::Transfer { amount } => Self::Transfer { amount },
            AdminCallAction::FunctionCall {
                method_name: function_name,
                args,
                deposit,
                gas,
            } => Self::FunctionCall {
                name: function_name,
                args,
                attached_yocto: deposit,
                gas,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ToString, format, vec};

    const RECEIVER: &str = "token.near";
    const TRANSFER: &str = r#"{"Transfer":{"amount":"1"}}"#;
    const CALL: &str =
        r#"{"FunctionCall":{"method_name":"f","args":"e30=","deposit":"2","gas":3}}"#;

    fn parse(json: &str) -> Result<AdminCallArgs, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Complete `admin_call` input with the given actions, each written as JSON.
    fn input(actions: &[&str]) -> String {
        let actions = actions.join(",");
        format!(r#"{{"receiver_id":"{RECEIVER}","actions":[{actions}]}}"#)
    }

    #[test]
    fn test_json_format() {
        let json = input(&[TRANSFER, CALL]);
        let args = AdminCallArgs {
            receiver_id: RECEIVER.parse().unwrap(),
            actions: vec![
                AdminCallAction::Transfer {
                    amount: Yocto::new(1),
                },
                AdminCallAction::FunctionCall {
                    method_name: "f".to_string(),
                    args: b"{}".to_vec(),
                    deposit: Yocto::new(2),
                    gas: NearGas::new(3),
                },
            ],
        };

        assert_eq!(parse(&json).unwrap(), args);
        assert_eq!(serde_json::to_string(&args).unwrap(), json);
    }

    #[test]
    fn test_function_call_args_are_optional() {
        let json = input(&[r#"{"FunctionCall":{"method_name":"f","deposit":"0","gas":1}}"#]);
        let args = parse(&json).unwrap();

        assert!(matches!(
            args.actions.as_slice(),
            [AdminCallAction::FunctionCall { args, .. }] if args.is_empty()
        ));
        // Empty `args` are left out again when serializing.
        assert_eq!(serde_json::to_string(&args).unwrap(), json);
    }

    #[test]
    fn test_accept_maximum_amounts() {
        // `u64::MAX` does not fit a double, so the gas must not be read through a float.
        let call = concat!(
            r#"{"FunctionCall":{"method_name":"f","#,
            r#""deposit":"340282366920938463463374607431768211455","gas":18446744073709551615}}"#,
        );
        let expected = AdminCallAction::FunctionCall {
            method_name: "f".to_string(),
            args: vec![],
            deposit: Yocto::new(u128::MAX),
            gas: NearGas::new(u64::MAX),
        };

        assert_eq!(parse(&input(&[call])).unwrap().actions, [expected]);
    }

    #[test]
    fn test_reject_unsupported_actions() {
        // Every other `PromiseAction`, as a bare name and as a variant with fields.
        let unsupported = [
            "CreateAccount",
            "DeployContract",
            "Stake",
            "AddFullAccessKey",
            "AddFunctionCallKey",
            "DeleteKey",
            "DeleteAccount",
        ];

        for name in unsupported {
            for action in [format!(r#""{name}""#), format!(r#"{{"{name}":{{}}}}"#)] {
                // An allowed action next to it does not make the batch acceptable.
                let err = parse(&input(&[TRANSFER, &action])).unwrap_err().to_string();
                assert!(err.contains("unknown variant"), "{action}: {err}");
            }
        }
    }

    #[test]
    fn test_reject_malformed_input() {
        let actions = [
            // Unknown and misspelt fields.
            r#"{"Transfer":{"amount":"1","memo":""}}"#,
            r#"{"FunctionCall":{"method_name":"f","arg":"e30=","deposit":"0","gas":1}}"#,
            // Missing fields.
            r#"{"Transfer":{}}"#,
            r#"{"FunctionCall":{"method_name":"f","deposit":"0"}}"#,
            // Wrong types, values out of range, invalid base64.
            r#"{"Transfer":{"amount":1}}"#,
            r#"{"Transfer":{"amount":"340282366920938463463374607431768211456"}}"#,
            r#"{"FunctionCall":{"method_name":"f","deposit":"0","gas":"1"}}"#,
            r#"{"FunctionCall":{"method_name":"f","deposit":"0","gas":18446744073709551616}}"#,
            r#"{"FunctionCall":{"method_name":"f","args":[123,125],"deposit":"0","gas":1}}"#,
            r#"{"FunctionCall":{"method_name":"f","args":"e30","deposit":"0","gas":1}}"#,
            // A repeated field.
            r#"{"Transfer":{"amount":"1","amount":"2"}}"#,
            // Not an object with exactly one variant.
            "{}",
            r#"{"Transfer":{"amount":"1"},"FunctionCall":{"method_name":"f","deposit":"0","gas":1}}"#,
        ];
        let inputs = [
            String::new(),
            format!("{} trailing", input(&[TRANSFER])),
            format!(r#"{{"actions":[{TRANSFER}]}}"#),
            format!(r#"{{"receiver_id":"{RECEIVER}"}}"#),
            format!(r#"{{"receiver_id":"Token.near","actions":[{TRANSFER}]}}"#),
        ];

        for json in actions.iter().map(|action| input(&[action])).chain(inputs) {
            assert!(parse(&json).is_err(), "{json}");
        }
    }
}

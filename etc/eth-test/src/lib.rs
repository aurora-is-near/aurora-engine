#[cfg(test)]
mod ttjson;
#[cfg(test)]
mod tests;


pub mod ttjson;
pub use crate::ttjson::tt_json_with_error::{TransactionTestJsonErr, TransactionTestErr, TransactonTestInfo, TtResultErr, TTErr};
pub use crate::ttjson::tt_json_with_success::{TransactionTestJsonOk, TransactionTestOk, TtResultOk, TTOk};

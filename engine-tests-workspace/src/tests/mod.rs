#[cfg(test)]
mod erc20;

use crate::prelude::U256;
use aurora_engine::parameters::NewCallArgs;
use aurora_engine_types::account_id::AccountId;
use near_units::parse_near;
use serde_json::json;
use workspaces::result::ExecutionFinalResult;
use workspaces::types::SecretKey;
use workspaces::Contract;

// pub struct Signer {
//     pub nonce: u64,
//     pub secret_key: SecretKey,
// }
//
// impl Signer {
//     pub fn new(secret_key: SecretKey) -> Self {
//         Self {
//             nonce: 0,
//             secret_key,
//         }
//     }
//
//     pub fn random() -> Self {
//         let mut rng = rand::thread_rng();
//         let sk = SecretKey::random(&mut rng);
//         Self::new(sk)
//     }
//
//     pub fn use_nonce(&mut self) -> u64 {
//         let nonce = self.nonce;
//         self.nonce += 1;
//         nonce
//     }
// }

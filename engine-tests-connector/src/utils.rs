use aurora_engine::parameters::{FungibleTokenMetadata, SetEthConnectorContractAccountArgs};
use aurora_engine::proof::Proof;
use aurora_engine_types::borsh::{self, BorshDeserialize, BorshSerialize};
use aurora_engine_types::parameters::connector::WithdrawSerializeType;
use aurora_engine_types::types::{Address, Wei};
use near_sdk::serde_json::json;
use near_sdk::{json_types::U128, serde_json};
use near_workspaces::network::NetworkClient;
use near_workspaces::types::NearToken;
use near_workspaces::{result::ExecutionFinalResult, Account, AccountId, Contract, Worker};
use std::path::Path;

pub const PROOF_DATA_NEAR: &str = r#"{"log_index":0,"log_entry_data":[248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,6,1,130,107,17,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"header_data":[249,2,10,160,177,33,112,26,26,176,12,12,163,2,249,133,245,12,51,201,55,50,148,156,122,67,27,26,101,178,36,153,54,100,53,137,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,124,28,230,160,8,239,64,193,62,78,177,68,166,204,116,240,224,174,172,126,160,197,65,5,202,188,134,5,164,246,19,133,35,57,28,114,241,186,81,123,163,166,161,24,32,157,168,170,13,108,58,61,46,160,6,199,163,13,91,119,225,39,168,255,213,10,107,252,143,246,138,241,108,139,59,35,187,185,162,223,53,108,222,73,181,109,160,27,154,49,63,26,170,15,177,97,255,6,204,84,221,234,197,159,172,114,47,148,126,32,199,241,127,101,120,182,51,52,100,185,1,0,0,0,8,0,0,0,0,0,0,0,32,0,0,0,0,0,2,0,8,0,0,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,8,32,0,32,0,0,128,0,2,0,0,0,1,0,32,0,0,0,2,0,0,0,0,32,0,0,0,0,0,4,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,128,64,0,0,0,0,1,32,0,0,0,0,0,0,96,32,0,64,0,0,0,128,1,0,0,0,0,1,0,0,0,8,0,0,0,18,32,0,0,64,145,1,8,0,4,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,33,16,0,128,0,0,0,0,0,0,128,0,2,0,0,0,0,0,0,0,0,0,0,2,0,80,0,0,0,0,0,0,0,0,1,128,0,8,0,0,0,0,4,0,0,0,128,2,0,32,0,128,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,16,0,8,0,0,0,0,0,0,0,0,0,0,128,0,64,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,132,25,1,227,23,131,157,85,14,131,122,18,0,131,75,91,132,132,96,174,58,224,140,115,112,105,100,101,114,49,48,1,2,8,230,160,188,212,199,183,154,22,223,85,103,215,24,122,240,235,79,129,44,93,184,88,161,218,79,5,44,226,106,100,50,40,163,97,136,155,158,202,3,149,91,200,78],"proof":[[248,113,160,46,156,31,85,241,226,241,13,5,56,73,146,176,67,195,109,6,189,172,104,44,103,44,88,32,15,181,152,136,29,121,252,160,191,48,87,174,71,151,208,114,164,150,51,200,171,90,90,106,46,200,79,77,222,145,95,89,141,137,138,149,67,73,8,87,128,128,128,128,128,128,160,175,9,219,77,174,13,247,133,55,172,92,185,202,7,160,10,204,112,44,133,36,96,30,234,235,134,30,209,205,166,212,255,128,128,128,128,128,128,128,128],[249,2,13,48,185,2,9,249,2,6,1,130,107,17,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0]]}"#;
pub const DEPOSITED_RECIPIENT: &str = "eth_recipient.root";
pub const DEPOSITED_RECIPIENT_NAME: &str = "eth_recipient";
pub const CUSTODIAN_ADDRESS: &str = "096DE9C2B8A5B8c22cEe3289B101f6960d68E51E";
pub const DEFAULT_GAS: near_gas::NearGas = near_gas::NearGas::from_tgas(300);
pub const DEPOSITED_AMOUNT: u128 = 800400;
pub const DEPOSITED_FEE: u128 = 400;
pub const RECIPIENT_ETH_ADDRESS: &str = "891b2749238b27ff58e951088e55b04de71dc374";
pub const PROOF_DATA_ETH: &str = r#"{"log_index":0,"log_entry_data":[249,1,27,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,39,216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,200,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,40,1,130,121,129,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,39,216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,200,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"header_data":[249,2,23,160,227,118,223,171,207,47,75,187,79,185,74,198,88,140,54,97,161,196,35,70,121,178,154,141,172,91,193,252,86,64,228,227,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,109,150,79,199,61,172,73,162,195,49,105,169,235,252,47,207,92,249,136,136,160,232,74,213,122,210,55,65,43,78,225,85,247,174,212,229,211,176,186,250,113,21,129,16,181,52,172,217,167,148,242,153,45,160,15,198,229,127,6,235,198,161,226,121,173,106,62,0,90,25,158,11,242,44,178,3,137,22,245,126,227,91,74,156,24,115,160,65,253,74,43,97,155,196,93,59,43,202,12,155,49,115,95,124,247,230,15,1,171,150,10,56,115,247,86,81,8,39,11,185,1,0,128,32,9,2,0,0,0,0,0,0,32,16,128,32,0,0,128,2,0,0,64,51,0,0,0,129,0,32,66,32,0,14,0,144,0,0,0,2,13,34,0,128,64,200,128,4,32,16,0,64,0,0,34,0,32,0,40,0,8,0,0,32,176,0,196,1,0,0,10,1,16,8,16,0,0,72,48,0,0,36,0,17,4,128,10,68,0,16,0,1,32,0,128,0,32,0,12,64,162,8,98,2,0,32,0,0,16,136,1,16,40,0,0,0,0,4,0,0,44,32,0,0,192,49,0,8,12,64,96,129,0,2,0,0,128,0,12,64,10,8,1,132,0,32,0,1,4,33,0,4,128,140,128,0,2,66,0,0,192,0,2,16,2,0,0,0,32,16,0,0,64,0,242,4,0,0,0,0,0,0,4,128,0,32,0,14,194,0,16,10,64,32,0,0,0,2,16,96,16,129,0,16,32,32,128,128,32,0,2,68,0,32,1,8,64,16,32,2,5,2,68,0,32,0,2,16,1,0,0,16,2,0,0,16,2,0,0,0,128,0,16,0,36,128,32,0,4,64,16,0,40,16,0,17,0,16,132,25,207,98,158,131,157,85,88,131,122,17,225,131,121,11,191,132,96,174,60,127,153,216,131,1,10,1,132,103,101,116,104,134,103,111,49,46,49,54,135,119,105,110,100,111,119,115,160,33,15,129,167,71,37,0,207,110,217,101,107,71,110,48,237,4,83,174,75,131,188,213,179,154,115,243,94,107,52,238,144,136,84,114,37,115,236,166,252,105],"proof":[[248,177,160,211,36,253,39,157,18,180,1,3,139,140,168,65,238,106,111,239,53,121,48,235,96,8,115,106,93,174,165,66,207,49,216,160,172,74,129,163,113,84,7,35,23,12,83,10,253,21,57,198,143,128,73,112,84,222,23,146,164,219,89,23,138,197,111,237,160,52,220,245,245,91,231,95,169,113,225,49,168,40,77,59,232,33,210,4,93,203,94,247,212,15,42,146,32,70,206,193,54,160,6,140,29,61,156,224,194,173,129,74,84,92,11,129,184,212,37,31,23,140,226,87,230,72,30,52,97,66,185,236,139,228,128,128,128,128,160,190,114,105,101,139,216,178,42,238,75,109,119,227,138,206,144,183,82,34,173,26,173,188,231,152,171,56,163,2,179,13,190,128,128,128,128,128,128,128,128],[249,2,47,48,185,2,43,249,2,40,1,130,121,129,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,39,216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,200,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0]]}"#;
pub const DEPOSITED_EVM_FEE: u128 = 200;
pub const DEPOSITED_EVM_AMOUNT: u128 = 10200;
pub const CONTRACT_ACC: &str = "eth_connector.root";

pub type PausedMask = u8;

/// Admin control flow flag indicates that all control flow unpause (unblocked).
pub const UNPAUSE_ALL: PausedMask = 0;
/// Admin control flow flag indicates that the deposit is paused.
pub const PAUSE_DEPOSIT: PausedMask = 1 << 0;
/// Admin control flow flag indicates that withdrawal is paused.
pub const PAUSE_WITHDRAW: PausedMask = 1 << 1;
/// Admin control flow flag indicates that ft transfers are paused.
pub const PAUSE_FT: PausedMask = 1 << 2;

pub struct TestContract {
    pub engine_contract: Contract,
    pub eth_connector_contract: Contract,
    pub root_account: Account,
}

impl TestContract {
    async fn deploy_aurora_contract() -> anyhow::Result<(Contract, Contract, Account)> {
        use near_workspaces::{
            types::{KeyType, SecretKey},
            AccessKey,
        };
        let worker = near_workspaces::sandbox()
            .await
            .map_err(|err| anyhow::anyhow!("Failed init sandbox: {:?}", err))?;
        let testnet = near_workspaces::testnet()
            .await
            .map_err(|err| anyhow::anyhow!("Failed init testnet: {:?}", err))?;
        let registrar: AccountId = "registrar".parse()?;
        let sk = SecretKey::from_seed(KeyType::ED25519, registrar.as_str());
        let registrar = worker
            .import_contract(&registrar, &testnet)
            .transact()
            .await?;
        Self::waiting_account_creation(&worker, registrar.id()).await?;

        let root: AccountId = "root".parse()?;
        registrar
            .as_account()
            .batch(&root)
            .create_account()
            .add_key(sk.public_key(), AccessKey::full_access())
            .transfer(NearToken::from_near(100))
            .transact()
            .await?
            .into_result()?;

        let root_account = Account::from_secret_key(root, sk, &worker);
        let eth_connector = root_account
            .create_subaccount("aurora_eth_connector")
            .initial_balance(NearToken::from_near(15))
            .transact()
            .await?
            .into_result()?;
        let engine = root_account
            .create_subaccount("eth_connector")
            .initial_balance(NearToken::from_near(15))
            .transact()
            .await?
            .into_result()?;
        let engine_contract_bytes = get_engine_contract();
        let engine_contract = engine.deploy(&engine_contract_bytes).await?.into_result()?;
        let eth_connector_contract = eth_connector
            .deploy(&get_eth_connector_contract())
            .await?
            .into_result()?;

        Ok((engine_contract, eth_connector_contract, root_account))
    }

    pub async fn new() -> anyhow::Result<Self> {
        Self::new_with_custodian(CUSTODIAN_ADDRESS).await
    }

    pub async fn new_with_owner(owner: AccountId) -> anyhow::Result<Self> {
        Self::new_contract(CUSTODIAN_ADDRESS, Some(owner)).await
    }

    pub async fn new_with_custodian(eth_custodian_address: &str) -> anyhow::Result<Self> {
        Self::new_contract(eth_custodian_address, None).await
    }

    async fn new_contract(
        eth_custodian_address: &str,
        owner: Option<AccountId>,
    ) -> anyhow::Result<Self> {
        let (engine_contract, eth_connector_contract, root_account) =
            Self::deploy_aurora_contract().await?;

        let prover_account: AccountId = eth_connector_contract.id().clone();
        let metadata = FungibleTokenMetadata::default();
        let account_with_access_right: AccountId = engine_contract.id().clone();
        // Init eth-connector
        let metadata = json!({
            "spec": metadata.spec,
            "name": metadata.name,
            "symbol": metadata.symbol,
            "icon": metadata.icon,
            "reference": metadata.reference,
            "decimals": metadata.decimals,
        });
        let owner_id = owner.unwrap_or_else(|| account_with_access_right.clone());
        let res = eth_connector_contract
            .call("new")
            .args_json(json!({
                "prover_account": prover_account,
                "eth_custodian_address": eth_custodian_address,
                "metadata": metadata,
                "account_with_access_right": account_with_access_right,
                "owner_id": owner_id,
                "min_proof_acceptance_height": 0,
            }))
            .gas(DEFAULT_GAS)
            .transact()
            .await?;
        assert!(res.is_success());

        let result = eth_connector_contract
            .call("pa_unpause_feature")
            .args_json(json!({ "key": "ALL" }))
            .max_gas()
            .transact()
            .await?;
        assert!(result.is_success(), "{result:#?}");

        let chain_id = [0u8; 32];
        let res = engine_contract
            .call("new")
            .args_borsh((chain_id, engine_contract.id(), engine_contract.id(), 1_u64))
            .gas(DEFAULT_GAS)
            .transact()
            .await?;
        assert!(res.is_success());

        let metadata = FungibleTokenMetadata::default();
        let res = engine_contract
            .call("new_eth_connector")
            .args_borsh((prover_account, eth_custodian_address, metadata))
            .gas(DEFAULT_GAS)
            .transact()
            .await?;
        assert!(res.is_success());

        let acc = SetEthConnectorContractAccountArgs {
            account: eth_connector_contract.id().as_str().parse().unwrap(),
            withdraw_serialize_type: WithdrawSerializeType::Borsh,
        };
        let res = engine_contract
            .call("set_eth_connector_contract_account")
            .args_borsh(acc)
            .gas(DEFAULT_GAS)
            .transact()
            .await?;
        assert!(res.is_success());

        Ok(Self {
            engine_contract,
            eth_connector_contract,
            root_account,
        })
    }

    /// Waiting for the account creation
    async fn waiting_account_creation<T: NetworkClient + ?Sized + Send + Sync>(
        worker: &Worker<T>,
        account_id: &AccountId,
    ) -> anyhow::Result<()> {
        let timer = std::time::Instant::now();
        // Try to get account within 30 secs
        for _ in 0..60 {
            if worker.view_account(account_id).await.is_err() {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            } else {
                return Ok(());
            }
        }

        anyhow::bail!(
            "Account `{}` was not created in {:?} sec",
            account_id,
            timer.elapsed()
        )
    }

    #[must_use]
    pub fn get_proof(&self, proof: &str) -> Proof {
        serde_json::from_str(proof).unwrap()
    }

    pub async fn create_sub_account(&self, name: &str) -> anyhow::Result<Account> {
        Ok(self
            .root_account
            .create_subaccount(name)
            .initial_balance(NearToken::from_near(15))
            .transact()
            .await?
            .into_result()?)
    }

    pub async fn deposit_with_proof(&self, proof: &Proof) -> anyhow::Result<ExecutionFinalResult> {
        Ok(self
            .engine_contract
            .call("deposit")
            .args_borsh(proof)
            .gas(DEFAULT_GAS)
            .transact()
            .await?)
    }

    pub async fn call_deposit_eth_to_near(&self) -> anyhow::Result<()> {
        let proof: Proof = self.get_proof(PROOF_DATA_NEAR);
        let res = self.deposit_with_proof(&proof).await?;
        assert!(res.is_success());
        Ok(())
    }

    pub async fn call_deposit_eth_to_aurora(&self) -> anyhow::Result<()> {
        let proof: Proof = serde_json::from_str(PROOF_DATA_ETH)?;
        let res = self.deposit_with_proof(&proof).await?;
        assert!(res.is_success());
        Ok(())
    }

    pub async fn user_deposit_with_proof(
        &self,
        user: &Account,
        proof: &Proof,
    ) -> anyhow::Result<ExecutionFinalResult> {
        Ok(user
            .call(self.engine_contract.id(), "deposit")
            .args_borsh(proof)
            .gas(DEFAULT_GAS)
            .transact()
            .await?)
    }

    #[must_use]
    pub fn check_error_message(&self, res: &ExecutionFinalResult, error_msg: &str) -> bool {
        let mut is_failure = false;
        for out in res.receipt_outcomes() {
            is_failure = out.is_failure();
            if is_failure {
                return format!("{res:?}").contains(error_msg);
            }
        }
        is_failure
    }

    pub async fn call_is_used_proof(&self, proof: &str) -> anyhow::Result<bool> {
        let proof: Proof = serde_json::from_str(proof)?;
        let res = self
            .engine_contract
            .call("is_used_proof")
            .args_borsh(proof)
            .gas(DEFAULT_GAS)
            .transact()
            .await?
            .into_result()?
            .borsh::<bool>()?;
        Ok(res)
    }

    pub async fn get_eth_on_near_balance(&self, account: &AccountId) -> anyhow::Result<U128> {
        let res = self
            .engine_contract
            .call("ft_balance_of")
            .args_json((account,))
            .gas(DEFAULT_GAS)
            .transact()
            .await?
            .into_result()?
            .json::<U128>()?;
        Ok(res)
    }

    pub async fn get_eth_balance(&self, address: &Address) -> anyhow::Result<u128> {
        #[derive(BorshSerialize, BorshDeserialize)]
        #[borsh(crate = "aurora_engine_types::borsh")]
        pub struct BalanceOfEthCallArgs {
            pub address: Address,
        }
        let args = borsh::to_vec(&BalanceOfEthCallArgs { address: *address })?;
        let res = self
            .engine_contract
            .call("ft_balance_of_eth")
            .args(args)
            .gas(DEFAULT_GAS)
            .transact()
            .await?;

        res.into_result()?
            .json::<Wei>()
            .map_err(Into::into)
            .and_then(|res| {
                res.try_into_u128()
                    .map_err(|e| anyhow::anyhow!(e.to_string()))
            })
    }

    pub async fn total_supply(&self) -> anyhow::Result<u128> {
        let res = self
            .engine_contract
            .call("ft_total_supply")
            .gas(DEFAULT_GAS)
            .transact()
            .await?
            .into_result()?
            .json::<U128>()?;
        Ok(res.0)
    }
}

pub fn print_logs(res: &ExecutionFinalResult) {
    for log in &res.logs() {
        println!("\t[LOG] {log}");
    }
}

#[must_use]
pub fn validate_eth_address(address: &str) -> Address {
    Address::decode(address).unwrap()
}

#[must_use]
pub fn get_eth_connector_contract() -> Vec<u8> {
    let contract_path = Path::new("etc/aurora-eth-connector");
    std::fs::read(contract_path.join("bin/aurora-eth-connector-test.wasm")).unwrap()
}

fn get_engine_contract() -> Vec<u8> {
    if cfg!(feature = "mainnet-test") {
        std::fs::read("../bin/aurora-mainnet-silo-test.wasm").unwrap()
    } else if cfg!(feature = "testnet-test") {
        std::fs::read("../bin/aurora-testnet-silo-test.wasm").unwrap()
    } else {
        panic!("AuroraRunner requires mainnet-test or testnet-test feature enabled.")
    }
}

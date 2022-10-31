use crate::prelude::Address;
use crate::prelude::WithdrawCallArgs;
use crate::test_utils::str_to_account_id;
use aurora_engine::admin_controlled::{PausedMask, ERR_PAUSED};
use aurora_engine::connector::{
    ERR_NOT_ENOUGH_BALANCE_FOR_FEE, PAUSE_DEPOSIT, PAUSE_WITHDRAW, UNPAUSE_ALL,
};
use aurora_engine::fungible_token::FungibleTokenMetadata;
use aurora_engine::parameters::{
    InitCallArgs, NewCallArgs, RegisterRelayerCallArgs, WithdrawResult,
};
use aurora_engine_types::types::{Fee, NEP141Wei};
use borsh::{BorshDeserialize, BorshSerialize};
use byte_slice_cast::AsByteSlice;
use ethabi::ethereum_types::U256;
use near_sdk::test_utils::accounts;
use near_sdk_sim::transaction::ExecutionStatus;
use near_sdk_sim::{to_yocto, ExecutionResult, UserAccount, DEFAULT_GAS, STORAGE_AMOUNT};
use serde_json::json;

const CONTRACT_ACC: &str = "eth_connector.root";
const PROOF_DATA_NEAR: &str = r#"{"log_index":0,"log_entry_data":[248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,6,1,130,107,17,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"header_data":[249,2,10,160,177,33,112,26,26,176,12,12,163,2,249,133,245,12,51,201,55,50,148,156,122,67,27,26,101,178,36,153,54,100,53,137,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,124,28,230,160,8,239,64,193,62,78,177,68,166,204,116,240,224,174,172,126,160,197,65,5,202,188,134,5,164,246,19,133,35,57,28,114,241,186,81,123,163,166,161,24,32,157,168,170,13,108,58,61,46,160,6,199,163,13,91,119,225,39,168,255,213,10,107,252,143,246,138,241,108,139,59,35,187,185,162,223,53,108,222,73,181,109,160,27,154,49,63,26,170,15,177,97,255,6,204,84,221,234,197,159,172,114,47,148,126,32,199,241,127,101,120,182,51,52,100,185,1,0,0,0,8,0,0,0,0,0,0,0,32,0,0,0,0,0,2,0,8,0,0,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,8,32,0,32,0,0,128,0,2,0,0,0,1,0,32,0,0,0,2,0,0,0,0,32,0,0,0,0,0,4,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,128,64,0,0,0,0,1,32,0,0,0,0,0,0,96,32,0,64,0,0,0,128,1,0,0,0,0,1,0,0,0,8,0,0,0,18,32,0,0,64,145,1,8,0,4,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,33,16,0,128,0,0,0,0,0,0,128,0,2,0,0,0,0,0,0,0,0,0,0,2,0,80,0,0,0,0,0,0,0,0,1,128,0,8,0,0,0,0,4,0,0,0,128,2,0,32,0,128,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,16,0,8,0,0,0,0,0,0,0,0,0,0,128,0,64,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,132,25,1,227,23,131,157,85,14,131,122,18,0,131,75,91,132,132,96,174,58,224,140,115,112,105,100,101,114,49,48,1,2,8,230,160,188,212,199,183,154,22,223,85,103,215,24,122,240,235,79,129,44,93,184,88,161,218,79,5,44,226,106,100,50,40,163,97,136,155,158,202,3,149,91,200,78],"proof":[[248,113,160,46,156,31,85,241,226,241,13,5,56,73,146,176,67,195,109,6,189,172,104,44,103,44,88,32,15,181,152,136,29,121,252,160,191,48,87,174,71,151,208,114,164,150,51,200,171,90,90,106,46,200,79,77,222,145,95,89,141,137,138,149,67,73,8,87,128,128,128,128,128,128,160,175,9,219,77,174,13,247,133,55,172,92,185,202,7,160,10,204,112,44,133,36,96,30,234,235,134,30,209,205,166,212,255,128,128,128,128,128,128,128,128],[249,2,13,48,185,2,9,249,2,6,1,130,107,17,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0]]}"#;
const PROOF_DATA_ETH: &str = r#"{"log_index":0,"log_entry_data":[249,1,27,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,39,216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,200,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,40,1,130,121,129,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,39,216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,200,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"header_data":[249,2,23,160,227,118,223,171,207,47,75,187,79,185,74,198,88,140,54,97,161,196,35,70,121,178,154,141,172,91,193,252,86,64,228,227,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,109,150,79,199,61,172,73,162,195,49,105,169,235,252,47,207,92,249,136,136,160,232,74,213,122,210,55,65,43,78,225,85,247,174,212,229,211,176,186,250,113,21,129,16,181,52,172,217,167,148,242,153,45,160,15,198,229,127,6,235,198,161,226,121,173,106,62,0,90,25,158,11,242,44,178,3,137,22,245,126,227,91,74,156,24,115,160,65,253,74,43,97,155,196,93,59,43,202,12,155,49,115,95,124,247,230,15,1,171,150,10,56,115,247,86,81,8,39,11,185,1,0,128,32,9,2,0,0,0,0,0,0,32,16,128,32,0,0,128,2,0,0,64,51,0,0,0,129,0,32,66,32,0,14,0,144,0,0,0,2,13,34,0,128,64,200,128,4,32,16,0,64,0,0,34,0,32,0,40,0,8,0,0,32,176,0,196,1,0,0,10,1,16,8,16,0,0,72,48,0,0,36,0,17,4,128,10,68,0,16,0,1,32,0,128,0,32,0,12,64,162,8,98,2,0,32,0,0,16,136,1,16,40,0,0,0,0,4,0,0,44,32,0,0,192,49,0,8,12,64,96,129,0,2,0,0,128,0,12,64,10,8,1,132,0,32,0,1,4,33,0,4,128,140,128,0,2,66,0,0,192,0,2,16,2,0,0,0,32,16,0,0,64,0,242,4,0,0,0,0,0,0,4,128,0,32,0,14,194,0,16,10,64,32,0,0,0,2,16,96,16,129,0,16,32,32,128,128,32,0,2,68,0,32,1,8,64,16,32,2,5,2,68,0,32,0,2,16,1,0,0,16,2,0,0,16,2,0,0,0,128,0,16,0,36,128,32,0,4,64,16,0,40,16,0,17,0,16,132,25,207,98,158,131,157,85,88,131,122,17,225,131,121,11,191,132,96,174,60,127,153,216,131,1,10,1,132,103,101,116,104,134,103,111,49,46,49,54,135,119,105,110,100,111,119,115,160,33,15,129,167,71,37,0,207,110,217,101,107,71,110,48,237,4,83,174,75,131,188,213,179,154,115,243,94,107,52,238,144,136,84,114,37,115,236,166,252,105],"proof":[[248,177,160,211,36,253,39,157,18,180,1,3,139,140,168,65,238,106,111,239,53,121,48,235,96,8,115,106,93,174,165,66,207,49,216,160,172,74,129,163,113,84,7,35,23,12,83,10,253,21,57,198,143,128,73,112,84,222,23,146,164,219,89,23,138,197,111,237,160,52,220,245,245,91,231,95,169,113,225,49,168,40,77,59,232,33,210,4,93,203,94,247,212,15,42,146,32,70,206,193,54,160,6,140,29,61,156,224,194,173,129,74,84,92,11,129,184,212,37,31,23,140,226,87,230,72,30,52,97,66,185,236,139,228,128,128,128,128,160,190,114,105,101,139,216,178,42,238,75,109,119,227,138,206,144,183,82,34,173,26,173,188,231,152,171,56,163,2,179,13,190,128,128,128,128,128,128,128,128],[249,2,47,48,185,2,43,249,2,40,1,130,121,129,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,39,216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,200,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0]]}"#;
const DEPOSITED_RECIPIENT: &str = "eth_recipient.root";
const PROVER_ACCOUNT: &str = "eth_connector.root";
const CUSTODIAN_ADDRESS: &str = "096DE9C2B8A5B8c22cEe3289B101f6960d68E51E";
const DEPOSITED_AMOUNT: u128 = 800400;
const DEPOSITED_FEE: u128 = 400;
const RECIPIENT_ETH_ADDRESS: &str = "891b2749238b27ff58e951088e55b04de71dc374";
const EVM_CUSTODIAN_ADDRESS: &str = "096DE9C2B8A5B8c22cEe3289B101f6960d68E51E";
const DEPOSITED_EVM_AMOUNT: u128 = 10200;
const DEPOSITED_EVM_FEE: u128 = 200;

#[derive(
    Default, BorshDeserialize, BorshSerialize, Clone, serde::Deserialize, serde::Serialize,
)]
pub struct Proof {
    pub log_index: u64,
    pub log_entry_data: Vec<u8>,
    pub receipt_index: u64,
    pub receipt_data: Vec<u8>,
    pub header_data: Vec<u8>,
    pub proof: Vec<Vec<u8>>,
}

#[derive(BorshDeserialize, Debug)]
pub struct IsUsedProofResult {
    pub is_used_proof: bool,
}

fn init(custodian_address: &str) -> (UserAccount, UserAccount) {
    let master_account = near_sdk_sim::init_simulator(None);
    let contract = init_contract(&master_account, CONTRACT_ACC, custodian_address);
    (master_account, contract)
}

fn init_contract(
    master_account: &UserAccount,
    contract_name: &str,
    custodian_address: &str,
) -> UserAccount {
    let contract_account = master_account.deploy(
        crate::test_utils::AuroraRunner::default().code.code(),
        contract_name.parse().unwrap(),
        to_yocto("1000000"),
    );
    contract_account
        .call(
            contract_name.parse().unwrap(),
            "new",
            &NewCallArgs {
                chain_id: [0u8; 32],
                owner_id: str_to_account_id(master_account.account_id.clone().as_str()),
                bridge_prover_id: str_to_account_id(accounts(0).as_str()),
                upgrade_delay_blocks: 1,
            }
            .try_to_vec()
            .unwrap(),
            DEFAULT_GAS,
            STORAGE_AMOUNT,
        )
        .assert_success();
    contract_account
        .call(
            contract_name.parse().unwrap(),
            "new_eth_connector",
            &InitCallArgs {
                prover_account: str_to_account_id(PROVER_ACCOUNT),
                eth_custodian_address: custodian_address.into(),
                metadata: FungibleTokenMetadata::default(),
            }
            .try_to_vec()
            .unwrap(),
            DEFAULT_GAS,
            0,
        )
        .assert_success();
    contract_account
}

fn validate_eth_address(address: &str) -> Address {
    Address::decode(address).unwrap()
}

fn call_deposit_eth_to_near(
    master_account: &UserAccount,
    contract: &str,
) -> Vec<Option<ExecutionResult>> {
    let proof: Proof = serde_json::from_str(PROOF_DATA_NEAR).unwrap();
    let res = master_account.call(
        contract.parse().unwrap(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    //println!("{:#?}", res.promise_results());
    // Calculate Gas burnt
    // let total_gas_burnt = res
    //     .promise_results()
    //     .iter()
    //     .fold(0, |s, v| s + v.as_ref().unwrap().gas_burnt());
    // println!("{:#?}", total_gas_burnt);
    res.promise_results()
}

fn call_is_used_proof(account: &UserAccount, contract: &str, proof: &str) -> bool {
    let proof: Proof = serde_json::from_str(proof).unwrap();
    let res = account.view(
        contract.parse().unwrap(),
        "is_used_proof",
        &proof.try_to_vec().unwrap(),
    );
    let result = IsUsedProofResult::try_from_slice(&res.unwrap()).unwrap();
    result.is_used_proof
}

fn assert_proof_was_used(account: &UserAccount, contract: &str, proof: &str) {
    let is_used_proof = call_is_used_proof(account, contract, proof);
    assert!(
        is_used_proof,
        "{}",
        "Expected not to fail because the proof should have been already used",
    );
}

fn assert_proof_was_not_used(account: &UserAccount, contract: &str, proof: &str) {
    let is_used_proof = call_is_used_proof(account, contract, proof);
    assert!(
        !is_used_proof,
        "{}",
        "Expected not to fail and to have an unused proof but it was already used",
    );
}

#[allow(dead_code)]
fn print_logs(logs: &[String]) {
    for l in logs {
        println!("[log] {}", l);
    }
}

fn call_deposit_eth_to_aurora(master_account: &UserAccount, contract: &str) {
    let proof: Proof = serde_json::from_str(PROOF_DATA_ETH).unwrap();
    let res = master_account.call(
        contract.parse().unwrap(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        10,
    );
    res.assert_success();
}

fn get_eth_on_near_balance(master_account: &UserAccount, acc: &str, contract: &str) -> u128 {
    #[derive(BorshSerialize)]
    pub struct BalanceOfCallArgs {
        pub account_id: String,
    }

    let balance = master_account.view(
        contract.parse().unwrap(),
        "ft_balance_of",
        json!({ "account_id": acc }).to_string().as_bytes(),
    );
    let val_str = String::from_utf8(balance.unwrap()).unwrap();
    let val = &val_str[1..val_str.len() - 1];
    val.parse().unwrap()
}

fn get_eth_balance(master_account: &UserAccount, address: Address, contract: &str) -> u128 {
    #[derive(BorshSerialize, BorshDeserialize)]
    pub struct BalanceOfEthCallArgs {
        pub address: Address,
    }

    let balance = master_account.view(
        contract.parse().unwrap(),
        "ft_balance_of_eth",
        &BalanceOfEthCallArgs { address }.try_to_vec().unwrap(),
    );
    let val_str = String::from_utf8(balance.unwrap()).unwrap();
    let val = &val_str[1..val_str.len() - 1];
    val.parse().unwrap()
}

fn total_supply(master_account: &UserAccount, contract: &str) -> u128 {
    let balance = master_account.view(contract.parse().unwrap(), "ft_total_supply", &[]);
    let val_str = String::from_utf8(balance.unwrap()).unwrap();
    let val = &val_str[1..val_str.len() - 1];
    val.parse().unwrap()
}

fn total_eth_supply_on_near(master_account: &UserAccount, contract: &str) -> u128 {
    let balance = master_account.view(
        contract.parse().unwrap(),
        "ft_total_eth_supply_on_near",
        &[],
    );
    let val_str = String::from_utf8(balance.unwrap()).unwrap();
    let val = &val_str[1..val_str.len() - 1];
    val.parse().unwrap()
}

fn total_eth_supply_on_aurora(master_account: &UserAccount, contract: &str) -> u128 {
    let balance = master_account.view(
        contract.parse().unwrap(),
        "ft_total_eth_supply_on_aurora",
        &[],
    );
    let val_str = String::from_utf8(balance.unwrap()).unwrap();
    let val = &val_str[1..val_str.len() - 1];
    val.parse().unwrap()
}

#[test]
fn test_deposit_eth_to_near_balance_total_supply() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_eth_to_near(&contract, CONTRACT_ACC);

    let balance = get_eth_on_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_eth_supply_on_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_eth_supply_on_aurora(&master_account, CONTRACT_ACC);
    assert_eq!(balance, 0);
}

#[test]
fn test_deposit_eth_to_aurora_balance_total_supply() {
    let (master_account, contract) = init(EVM_CUSTODIAN_ADDRESS);
    let res = contract.call(
        CONTRACT_ACC.parse().unwrap(),
        "register_relayer",
        &RegisterRelayerCallArgs {
            address: validate_eth_address(CUSTODIAN_ADDRESS),
        }
        .try_to_vec()
        .unwrap(),
        DEFAULT_GAS,
        0,
    );
    res.assert_success();

    call_deposit_eth_to_aurora(&contract, CONTRACT_ACC);
    assert_proof_was_used(&contract, CONTRACT_ACC, PROOF_DATA_ETH);

    let balance = get_eth_balance(
        &master_account,
        validate_eth_address(RECIPIENT_ETH_ADDRESS),
        CONTRACT_ACC,
    );
    assert_eq!(balance, DEPOSITED_EVM_AMOUNT - DEPOSITED_EVM_FEE);

    let balance = get_eth_balance(
        &master_account,
        validate_eth_address(CUSTODIAN_ADDRESS),
        CONTRACT_ACC,
    );
    assert_eq!(balance, DEPOSITED_EVM_FEE);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_EVM_AMOUNT);

    let balance = total_eth_supply_on_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_EVM_AMOUNT);

    let balance = total_eth_supply_on_aurora(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_EVM_AMOUNT);
}

#[test]
fn test_withdraw_eth_from_near() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_eth_to_near(&contract, CONTRACT_ACC);

    let withdraw_amount = NEP141Wei::new(100);
    let recipient_addr = validate_eth_address(RECIPIENT_ETH_ADDRESS);
    let res = contract.call(
        CONTRACT_ACC.parse().unwrap(),
        "withdraw",
        &WithdrawCallArgs {
            recipient_address: recipient_addr,
            amount: withdraw_amount,
        }
        .try_to_vec()
        .unwrap(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();
    let data = res.promise_results();
    assert!(data.len() > 1);
    assert!(data[0].is_some());
    match data[1].clone().unwrap().outcome().status {
        ExecutionStatus::SuccessValue(ref v) => {
            let d: WithdrawResult = WithdrawResult::try_from_slice(v).unwrap();
            assert_eq!(d.amount, withdraw_amount);
            assert_eq!(d.recipient_id, recipient_addr);
            let custodian_addr = validate_eth_address(CUSTODIAN_ADDRESS);
            assert_eq!(d.eth_custodian_address, custodian_addr);
        }
        _ => panic!(),
    }

    let balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE - withdraw_amount.as_u128());

    let balance = get_eth_on_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - withdraw_amount.as_u128());
}

#[test]
fn test_ft_transfer() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_eth_to_near(&contract, CONTRACT_ACC);

    let transfer_amount = 70;
    let res = contract.call(
        CONTRACT_ACC.parse().unwrap(),
        "ft_transfer",
        json!({
            "receiver_id": DEPOSITED_RECIPIENT,
            "amount": transfer_amount.to_string(),
            "memo": "transfer memo"
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();

    let balance = get_eth_on_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(
        balance,
        DEPOSITED_AMOUNT - DEPOSITED_FEE + transfer_amount as u128
    );

    let balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE - transfer_amount as u128);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_eth_supply_on_aurora(&master_account, CONTRACT_ACC);
    assert_eq!(balance, 0);

    let balance = total_eth_supply_on_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);
}

#[test]
fn test_ft_transfer_call_eth() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_eth_to_near(&contract, CONTRACT_ACC);

    let balance = get_eth_on_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE);

    let res = contract.call(
        CONTRACT_ACC.parse().unwrap(),
        "register_relayer",
        &RegisterRelayerCallArgs {
            address: validate_eth_address(CUSTODIAN_ADDRESS),
        }
        .try_to_vec()
        .unwrap(),
        DEFAULT_GAS,
        0,
    );
    res.assert_success();

    let transfer_amount = 50;
    let fee: u128 = 30;
    let mut msg = U256::from(fee).as_byte_slice().to_vec();
    msg.append(
        &mut validate_eth_address(RECIPIENT_ETH_ADDRESS)
            .as_bytes()
            .to_vec(),
    );

    let message = [CONTRACT_ACC, hex::encode(msg).as_str()].join(":");
    let res = contract.call(
        CONTRACT_ACC.parse().unwrap(),
        "ft_transfer_call",
        json!({
            "receiver_id": CONTRACT_ACC,
            "amount": transfer_amount.to_string(),
            "msg": message,
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();

    let balance = get_eth_on_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE);

    let balance = get_eth_balance(
        &master_account,
        validate_eth_address(RECIPIENT_ETH_ADDRESS),
        CONTRACT_ACC,
    );
    assert_eq!(balance, transfer_amount - fee);

    let balance = get_eth_balance(
        &master_account,
        validate_eth_address(CUSTODIAN_ADDRESS),
        CONTRACT_ACC,
    );
    assert_eq!(balance, fee);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_eth_supply_on_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_eth_supply_on_aurora(&master_account, CONTRACT_ACC);
    assert_eq!(balance, transfer_amount);
}

#[test]
fn test_ft_transfer_call_without_message() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    let recipient_account = create_user_account(&master_account);
    call_deposit_eth_to_near(&contract, CONTRACT_ACC);

    let balance = get_eth_on_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE);

    let res = contract.call(
        CONTRACT_ACC.parse().unwrap(),
        "register_relayer",
        &RegisterRelayerCallArgs {
            address: validate_eth_address(CUSTODIAN_ADDRESS),
        }
        .try_to_vec()
        .unwrap(),
        DEFAULT_GAS,
        0,
    );
    res.assert_success();

    let transfer_amount = 50;
    // Send to Aurora contract with wrong message should failed
    let res = contract.call(
        CONTRACT_ACC.parse().unwrap(),
        "ft_transfer_call",
        json!({
            "receiver_id": CONTRACT_ACC,
            "amount": transfer_amount.to_string(),
            "msg": "",
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    match res.outcome().status {
        ExecutionStatus::Failure(_) => {}
        _ => panic!("Expected execution failure"),
    }

    // Assert balances remain unchanged
    let balance = get_eth_on_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);
    let balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE);

    // Sending to random account should not change balances
    let transfer_amount = 22;
    let res = recipient_account.call(
        CONTRACT_ACC.parse().unwrap(),
        "ft_transfer_call",
        json!({
            "receiver_id": "some-test-acc",
            "amount": transfer_amount.to_string(),
            "msg": "",
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();

    // some-test-acc does not implement `ft_on_transfer` therefore the call fails and the transfer is reverted.
    let balance = get_eth_on_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);
    let balance = get_eth_on_near_balance(&master_account, "some-test-acc", CONTRACT_ACC);
    assert_eq!(balance, 0);
    let balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE);

    // Sending to external receiver with empty message should be success
    let dummy_ft_receiver = master_account.deploy(
        &dummy_ft_receiver_bytes(),
        "ft-rec.root".parse().unwrap(),
        near_sdk_sim::STORAGE_AMOUNT,
    );
    let res = recipient_account.call(
        CONTRACT_ACC.parse().unwrap(),
        "ft_transfer_call",
        json!({
            "receiver_id": dummy_ft_receiver.account_id(),
            "amount": transfer_amount.to_string(),
            "msg": "",
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();

    let balance = get_eth_on_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE - transfer_amount);
    let balance = get_eth_on_near_balance(
        &master_account,
        dummy_ft_receiver.account_id().as_ref(),
        CONTRACT_ACC,
    );
    assert_eq!(balance, transfer_amount);
    let balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE);

    let balance = get_eth_balance(
        &master_account,
        validate_eth_address(RECIPIENT_ETH_ADDRESS),
        CONTRACT_ACC,
    );
    assert_eq!(balance, 0);

    let balance = get_eth_balance(
        &master_account,
        validate_eth_address(CUSTODIAN_ADDRESS),
        CONTRACT_ACC,
    );
    assert_eq!(balance, 0);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_eth_supply_on_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_eth_supply_on_aurora(&master_account, CONTRACT_ACC);
    assert_eq!(balance, 0);
}

#[test]
fn test_deposit_with_0x_prefix() {
    use aurora_engine::deposit_event::TokenMessageData;
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);

    let eth_custodian_address: Address = Address::decode(CUSTODIAN_ADDRESS).unwrap();
    let recipient_address = Address::from_array([10u8; 20]);
    let deposit_amount = 17;
    let recipient_address_encoded = recipient_address.encode();

    // Note the 0x prefix before the deposit address.
    let message = [CONTRACT_ACC, ":", "0x", &recipient_address_encoded].concat();
    let fee: Fee = Fee::new(NEP141Wei::new(0));
    let token_message_data =
        TokenMessageData::parse_event_message_and_prepare_token_message_data(&message, fee)
            .unwrap();

    let deposit_event = aurora_engine::deposit_event::DepositedEvent {
        eth_custodian_address,
        sender: Address::zero(),
        token_message_data,
        amount: NEP141Wei::new(deposit_amount),
        fee,
    };

    let event_schema = ethabi::Event {
        name: aurora_engine::deposit_event::DEPOSITED_EVENT.into(),
        inputs: aurora_engine::deposit_event::DepositedEvent::event_params(),
        anonymous: false,
    };
    let log_entry = aurora_engine::log_entry::LogEntry {
        address: eth_custodian_address.raw(),
        topics: vec![
            event_schema.signature(),
            // the sender is not important
            crate::prelude::H256::zero(),
        ],
        data: ethabi::encode(&[
            ethabi::Token::String(message),
            ethabi::Token::Uint(U256::from(deposit_event.amount.as_u128())),
            ethabi::Token::Uint(U256::from(deposit_event.fee.as_u128())),
        ]),
    };
    let proof = Proof {
        log_index: 1,
        // Only this field matters for the purpose of this test
        log_entry_data: rlp::encode(&log_entry).to_vec(),
        receipt_index: 1,
        receipt_data: Vec::new(),
        header_data: Vec::new(),
        proof: Vec::new(),
    };

    let res = master_account.call(
        contract.account_id(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    res.assert_success();

    let aurora_balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(aurora_balance, deposit_amount);
    let address_balance = get_eth_balance(&master_account, recipient_address, CONTRACT_ACC);
    assert_eq!(address_balance, deposit_amount);
}

fn generate_dummy_proof(message: String, deposit_amount: u128, log_index: u64) -> Proof {
    use aurora_engine::deposit_event::TokenMessageData;

    let eth_custodian_address: Address = Address::decode(CUSTODIAN_ADDRESS).unwrap();

    let fee: Fee = Fee::new(NEP141Wei::new(0));
    let token_message_data =
        TokenMessageData::parse_event_message_and_prepare_token_message_data(&message, fee)
            .unwrap();

    let deposit_event = aurora_engine::deposit_event::DepositedEvent {
        eth_custodian_address,
        sender: Address::zero(),
        token_message_data,
        amount: NEP141Wei::new(deposit_amount),
        fee,
    };

    let event_schema = ethabi::Event {
        name: aurora_engine::deposit_event::DEPOSITED_EVENT.into(),
        inputs: aurora_engine::deposit_event::DepositedEvent::event_params(),
        anonymous: false,
    };
    let log_entry = aurora_engine::log_entry::LogEntry {
        address: eth_custodian_address.raw(),
        topics: vec![
            event_schema.signature(),
            // the sender is not important
            crate::prelude::H256::zero(),
        ],
        data: ethabi::encode(&[
            ethabi::Token::String(message),
            ethabi::Token::Uint(U256::from(deposit_event.amount.as_u128())),
            ethabi::Token::Uint(U256::from(deposit_event.fee.as_u128())),
        ]),
    };

    Proof {
        log_index,
        // Only this field matters for the purpose of this test
        log_entry_data: rlp::encode(&log_entry).to_vec(),
        receipt_index: 1,
        receipt_data: Vec::new(),
        header_data: Vec::new(),
        proof: Vec::new(),
    }
}

#[test]
fn test_deposit_eth_to_near_account() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);

    let deposit_amount = 17;

    let user_account_id = "some_user.root";
    let _user_account = master_account.create_user(
        user_account_id.parse().unwrap(),
        to_yocto("100"), // initial balance
    );
    let proof = generate_dummy_proof(user_account_id.to_string(), deposit_amount, 1);

    let res = master_account.call(
        contract.account_id(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    let promises = res.promise_results();
    for p in promises.iter() {
        assert!(p.is_some());
        let p = p.as_ref().unwrap();
        p.assert_success()
    }
    res.assert_success();

    let aurora_balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(aurora_balance, 0);
    let user_account_balance =
        get_eth_on_near_balance(&master_account, user_account_id, CONTRACT_ACC);
    assert_eq!(user_account_balance, deposit_amount);
}

#[test]
fn test_deposit_eth_with_empty_custom_connector_account() {
    // In this, test we make an ETH deposit using the message format for targeting
    // an Aurora address, but use a different NEAR account than the Aurora Engine.
    // The result is that the ETH is correctly minted to the Engine, but then an
    // error occurs when it tries to transfer those funds because the listed NEAR
    // account does not implement `ft_on_transfer`.
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);

    let deposit_amount = 17;

    let user_account_id = "some_user.root";
    let _user_account = master_account.create_user(
        user_account_id.parse().unwrap(),
        to_yocto("100"), // initial balance
    );

    let recipient_address = Address::from_array([10u8; 20]);
    let recipient_address_encoded = recipient_address.encode();
    let message = [user_account_id, ":", "0x", &recipient_address_encoded].concat();
    let proof = generate_dummy_proof(message, deposit_amount, 1);
    let res = master_account.call(
        contract.account_id(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    let promises = res.promise_results();
    res.assert_success();

    let promise = &promises[promises.len() - 5];
    assert_execution_status_failure(
        promise.as_ref().unwrap().outcome().clone().status,
        format!(
            r#"CompilationError(CodeDoesNotExist {{ account_id: AccountId("{}") }}"#,
            user_account_id
        )
        .as_str(),
        "Expected failure in `ft_on_transfer` call, but deposit succeeded",
    );

    let user_account_balance =
        get_eth_on_near_balance(&master_account, user_account_id, CONTRACT_ACC);
    assert_eq!(user_account_balance, 0);
    let aurora_balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(aurora_balance, deposit_amount);
    let address_balance = get_eth_balance(&master_account, recipient_address, CONTRACT_ACC);
    assert_eq!(address_balance, 0);
}

#[test]
fn test_deposit_eth_with_custom_connector_account() {
    // In this test, we make an ETH deposit using the message format for targeting
    // an Aurora address, but use a different NEAR account than the Aurora Engine.
    // Additionally, the target account implements `ft_on_transfer` so that it can
    // receive the ETH and perform some action with it. This is safe because the ETH is
    // minted in the Engine first, then transferred to the target account using
    // `ft_transfer_call`.
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);

    let deposit_amount = 17;

    let user_account_id = "some_user.root";
    let _user_account = master_account.deploy(
        &dummy_ft_receiver_bytes(),
        user_account_id.parse().unwrap(),
        to_yocto("100"), // initial balance
    );

    let recipient_address = Address::from_array([10u8; 20]);
    let recipient_address_encoded = recipient_address.encode();
    let message = [user_account_id, ":", "0x", &recipient_address_encoded].concat();
    let proof = generate_dummy_proof(message, deposit_amount, 1);
    let res = master_account.call(
        contract.account_id(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    let promises = res.promise_results();
    for p in promises.iter() {
        assert!(p.is_some());
        let p = p.as_ref().unwrap();
        if p.executor_id().as_str() == user_account_id {
            // The `ft_on_transfer` implementation in the user's account generates this log.
            assert_eq!(
                p.logs().first().map(|s| s.as_str()),
                Some("in 17 tokens from @eth_connector.root ft_on_transfer, msg = some_user.root:00000000000000000000000000000000000000000000000000000000000000000a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a"),
            );
        }
        p.assert_success()
    }
    res.assert_success();

    let user_account_balance =
        get_eth_on_near_balance(&master_account, user_account_id, CONTRACT_ACC);
    assert_eq!(user_account_balance, deposit_amount);
    let aurora_balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(aurora_balance, 0);
    let address_balance = get_eth_balance(&master_account, recipient_address, CONTRACT_ACC);
    assert_eq!(address_balance, 0);
}

#[test]
fn test_deposit_with_same_proof() {
    let (_master_account, contract) = init(CUSTODIAN_ADDRESS);

    assert_proof_was_not_used(&contract, CONTRACT_ACC, PROOF_DATA_NEAR);

    let promises = call_deposit_eth_to_near(&contract, CONTRACT_ACC);
    for p in promises.iter() {
        assert!(p.is_some());
        let p = p.as_ref().unwrap();
        p.assert_success()
    }

    assert_proof_was_used(&contract, CONTRACT_ACC, PROOF_DATA_NEAR);

    let promises = call_deposit_eth_to_near(&contract, CONTRACT_ACC);
    let promise = &promises[promises.len() - 2];
    assert_execution_status_failure(
        promise.as_ref().unwrap().outcome().clone().status,
        "ERR_PROOF_EXIST",
        "Expected failure as the provided proof already exists, but deposit succeeded",
    );
}

#[test]
fn test_deposit_wrong_custodian_address() {
    let wrong_custodian_address = "0000000000000000000000000000000000000001";
    let (_master_account, contract) = init(wrong_custodian_address);
    let promises = call_deposit_eth_to_near(&contract, CONTRACT_ACC);
    let promise = &promises[promises.len() - 2];
    assert_execution_status_failure(
        promise.as_ref().unwrap().outcome().clone().status,
        "ERR_WRONG_EVENT_ADDRESS",
        "Expected failure as the provided proof originated from wrong EthCustodian contract, but deposit succeeded",
    );

    assert_proof_was_not_used(&contract, CONTRACT_ACC, PROOF_DATA_NEAR);
}

#[test]
fn test_ft_transfer_call_without_relayer() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_eth_to_near(&contract, CONTRACT_ACC);

    let balance = get_eth_on_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE);

    let transfer_amount = 50;
    let fee: u128 = 30;
    let mut msg = U256::from(fee).as_byte_slice().to_vec();
    msg.append(
        &mut validate_eth_address(RECIPIENT_ETH_ADDRESS)
            .as_bytes()
            .to_vec(),
    );
    let relayer_id = "relayer.root";
    let message = [relayer_id, hex::encode(msg).as_str()].join(":");
    let res = contract.call(
        CONTRACT_ACC.parse().unwrap(),
        "ft_transfer_call",
        json!({
            "receiver_id": CONTRACT_ACC,
            "amount": transfer_amount.to_string(),
            "msg": message,
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();

    let balance = get_eth_on_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE);

    let balance = get_eth_balance(
        &master_account,
        validate_eth_address(RECIPIENT_ETH_ADDRESS),
        CONTRACT_ACC,
    );
    assert_eq!(balance, transfer_amount);

    let balance = get_eth_balance(
        &master_account,
        validate_eth_address(CUSTODIAN_ADDRESS),
        CONTRACT_ACC,
    );
    assert_eq!(balance, 0);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_eth_supply_on_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_eth_supply_on_aurora(&master_account, CONTRACT_ACC);
    assert_eq!(balance, transfer_amount);
}

#[test]
fn test_ft_transfer_call_fee_greater_than_amount() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_eth_to_near(&contract, CONTRACT_ACC);

    let transfer_amount = 10;
    let fee: u128 = transfer_amount + 10;
    let mut msg = fee.to_be_bytes().to_vec();
    msg.append(
        &mut validate_eth_address(RECIPIENT_ETH_ADDRESS)
            .as_bytes()
            .to_vec(),
    );
    let relayer_id = "relayer.root";
    let message = [relayer_id, hex::encode(msg).as_str()].join(":");
    let res = contract.call(
        CONTRACT_ACC.parse().unwrap(),
        "ft_transfer_call",
        json!({
            "receiver_id": CONTRACT_ACC,
            "amount": transfer_amount.to_string(),
            "msg": message,
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    match res.outcome().clone().status {
        ExecutionStatus::Failure(_) => {}
        _ => panic!(),
    }

    let balance = get_eth_on_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE);

    let balance = get_eth_balance(
        &master_account,
        validate_eth_address(RECIPIENT_ETH_ADDRESS),
        CONTRACT_ACC,
    );
    assert_eq!(balance, 0);

    let balance = get_eth_balance(
        &master_account,
        validate_eth_address(CUSTODIAN_ADDRESS),
        CONTRACT_ACC,
    );
    assert_eq!(balance, 0);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_eth_supply_on_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_eth_supply_on_aurora(&master_account, CONTRACT_ACC);
    assert_eq!(balance, 0);
}

fn call_deposit_with_proof(
    account: &UserAccount,
    contract: &str,
    proof: &str,
) -> Vec<Option<ExecutionResult>> {
    let proof: Proof = serde_json::from_str(proof).unwrap();
    let res = account.call(
        contract.parse().unwrap(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    res.promise_results()
}

fn call_set_paused_flags(
    account: &UserAccount,
    contract: &str,
    paused_mask: PausedMask,
) -> ExecutionResult {
    account.call(
        contract.parse().unwrap(),
        "set_paused_flags",
        &paused_mask.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    )
}

fn create_user_account(master_account: &UserAccount) -> UserAccount {
    master_account.create_user(
        "eth_recipient.root".parse().unwrap(),
        to_yocto("100"), // initial balance
    )
}

#[test]
fn test_admin_controlled_only_admin_can_pause() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    let user_account = create_user_account(&master_account);

    // Try to pause from the user - should fail
    let res = call_set_paused_flags(&user_account, CONTRACT_ACC, PAUSE_DEPOSIT);
    let promises = res.promise_results();
    let p = promises[1].clone();
    match p.unwrap().outcome().clone().status {
        ExecutionStatus::Failure(_) => {}
        _ => panic!("Expected failure as only admin can pause, but user successfully paused"),
    }

    // Try to pause from the admin - should succeed
    let res = call_set_paused_flags(&contract, CONTRACT_ACC, PAUSE_DEPOSIT);
    res.assert_success();
}

#[test]
fn test_admin_controlled_admin_can_peform_actions_when_paused() {
    let (_master_account, contract) = init(CUSTODIAN_ADDRESS);

    // 1st deposit call when unpaused - should succeed
    let promises = call_deposit_with_proof(&contract, CONTRACT_ACC, PROOF_DATA_NEAR);
    for p in promises.iter() {
        assert!(p.is_some());
        let p = p.as_ref().unwrap();
        p.assert_success()
    }

    let withdraw_amount = NEP141Wei::new(100);
    let recipient_addr = validate_eth_address(RECIPIENT_ETH_ADDRESS);

    // 1st withdraw call when unpaused  - should succeed
    let res = contract.call(
        CONTRACT_ACC.parse().unwrap(),
        "withdraw",
        &WithdrawCallArgs {
            recipient_address: recipient_addr,
            amount: withdraw_amount,
        }
        .try_to_vec()
        .unwrap(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();
    let promises = res.promise_results();
    for p in promises.iter() {
        assert!(p.is_some());
        let p = p.as_ref().unwrap();
        p.assert_success()
    }

    // Pause deposit
    let res = call_set_paused_flags(&contract, CONTRACT_ACC, PAUSE_DEPOSIT);
    res.assert_success();

    // 2nd deposit call when paused, but the admin is calling it - should succeed
    // NB: We can use `PROOF_DATA_ETH` this will be just a different proof but the same deposit
    // method which should be paused
    let promises = call_deposit_with_proof(&contract, CONTRACT_ACC, PROOF_DATA_ETH);
    for p in promises.iter() {
        assert!(p.is_some());
        let p = p.as_ref().unwrap();
        p.assert_success()
    }

    // Pause withdraw
    let res = call_set_paused_flags(&contract, CONTRACT_ACC, PAUSE_WITHDRAW);
    res.assert_success();

    // 2nd withdraw call when paused, but the admin is calling it - should succeed
    let res = contract.call(
        CONTRACT_ACC.parse().unwrap(),
        "withdraw",
        &WithdrawCallArgs {
            recipient_address: recipient_addr,
            amount: withdraw_amount,
        }
        .try_to_vec()
        .unwrap(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();
    let promises = res.promise_results();
    for p in promises.iter() {
        assert!(p.is_some());
        let p = p.as_ref().unwrap();
        p.assert_success()
    }
}

#[test]
fn test_deposit_pausability() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    let user_account = create_user_account(&master_account);

    // 1st deposit call - should succeed
    let promises = call_deposit_with_proof(&user_account, CONTRACT_ACC, PROOF_DATA_NEAR);
    for p in promises.iter() {
        assert!(p.is_some());
        let p = p.as_ref().unwrap();
        p.assert_success()
    }

    // Pause deposit
    let res = call_set_paused_flags(&contract, CONTRACT_ACC, PAUSE_DEPOSIT);
    res.assert_success();

    // 2nd deposit call - should fail
    // NB: We can use `PROOF_DATA_ETH` this will be just a different proof but the same deposit
    // method which should be paused
    let promises = call_deposit_with_proof(&user_account, CONTRACT_ACC, PROOF_DATA_ETH);
    let num_promises = promises.len();
    let p = promises[num_promises - 2].clone();
    assert_execution_status_failure(
        p.unwrap().outcome().clone().status,
        ERR_PAUSED,
        "Expected failure due to pause, but deposit succeeded",
    );

    // Unpause all
    let res = call_set_paused_flags(&contract, CONTRACT_ACC, UNPAUSE_ALL);
    res.assert_success();

    // 3rd deposit call - should succeed
    let promises = call_deposit_with_proof(&user_account, CONTRACT_ACC, PROOF_DATA_ETH);
    for p in promises.iter() {
        assert!(p.is_some());
        let p = p.as_ref().unwrap();
        p.assert_success()
    }
}

#[test]
fn test_withdraw_from_near_pausability() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    let user_account = create_user_account(&master_account);

    call_deposit_eth_to_near(&contract, CONTRACT_ACC);

    let withdraw_amount = NEP141Wei::new(100);
    let recipient_addr = validate_eth_address(RECIPIENT_ETH_ADDRESS);
    // 1st withdraw - should succeed
    let res = user_account.call(
        CONTRACT_ACC.parse().unwrap(),
        "withdraw",
        &WithdrawCallArgs {
            recipient_address: recipient_addr,
            amount: withdraw_amount,
        }
        .try_to_vec()
        .unwrap(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();
    let promises = res.promise_results();
    assert!(promises.len() > 1);
    for p in promises.iter() {
        assert!(p.is_some());
        let p = p.as_ref().unwrap();
        p.assert_success()
    }

    // Pause withdraw
    let res = call_set_paused_flags(&contract, CONTRACT_ACC, PAUSE_WITHDRAW);
    res.assert_success();

    // 2nd withdraw - should fail
    let res = user_account.call(
        CONTRACT_ACC.parse().unwrap(),
        "withdraw",
        &WithdrawCallArgs {
            recipient_address: recipient_addr,
            amount: withdraw_amount,
        }
        .try_to_vec()
        .unwrap(),
        DEFAULT_GAS,
        1,
    );
    let promises = res.promise_results();
    let p = promises[1].clone();
    assert_execution_status_failure(
        p.unwrap().outcome().clone().status,
        ERR_PAUSED,
        "Expected failure due to pause, but withdraw succeeded",
    );

    // Unpause all
    let res = call_set_paused_flags(&contract, CONTRACT_ACC, UNPAUSE_ALL);
    res.assert_success();

    let res = user_account.call(
        CONTRACT_ACC.parse().unwrap(),
        "withdraw",
        &WithdrawCallArgs {
            recipient_address: recipient_addr,
            amount: withdraw_amount,
        }
        .try_to_vec()
        .unwrap(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();
    let promises = res.promise_results();
    assert!(promises.len() > 1);
    for p in promises.iter() {
        assert!(p.is_some());
        let p = p.as_ref().unwrap();
        p.assert_success()
    }
}

#[test]
fn test_get_accounts_counter() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_eth_to_near(&contract, CONTRACT_ACC);

    let counter = master_account
        .view(CONTRACT_ACC.parse().unwrap(), "get_accounts_counter", &[])
        .unwrap();
    assert_eq!(u64::try_from_slice(&counter[..]).unwrap(), 2);
}

#[test]
fn test_get_accounts_counter_and_transfer() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_eth_to_near(&contract, CONTRACT_ACC);

    let counter = master_account
        .view(CONTRACT_ACC.parse().unwrap(), "get_accounts_counter", &[])
        .unwrap();
    assert_eq!(u64::try_from_slice(&counter[..]).unwrap(), 2);

    let transfer_amount = 70;
    let res = contract.call(
        CONTRACT_ACC.parse().unwrap(),
        "ft_transfer",
        json!({
            "receiver_id": DEPOSITED_RECIPIENT,
            "amount": transfer_amount.to_string(),
            "memo": "transfer memo"
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();

    let balance = get_eth_on_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(
        balance,
        DEPOSITED_AMOUNT - DEPOSITED_FEE + transfer_amount as u128
    );

    let balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE - transfer_amount as u128);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_eth_supply_on_aurora(&master_account, CONTRACT_ACC);
    assert_eq!(balance, 0);

    let balance = total_eth_supply_on_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let counter = master_account
        .view(CONTRACT_ACC.parse().unwrap(), "get_accounts_counter", &[])
        .unwrap();
    assert_eq!(u64::try_from_slice(&counter[..]).unwrap(), 2);
}

#[test]
fn test_deposit_to_near_with_zero_fee() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    let proof_str = r#"{"log_index":0,"log_entry_data":[248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,11,184,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,6,1,130,106,249,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,11,184,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"header_data":[249,2,23,160,7,139,123,21,146,99,81,234,117,153,151,30,67,221,231,90,105,219,121,127,196,224,201,83,178,31,173,155,190,123,227,174,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,109,150,79,199,61,172,73,162,195,49,105,169,235,252,47,207,92,249,136,136,160,227,202,170,144,85,104,169,90,220,93,227,155,76,252,229,223,163,146,127,223,157,121,27,238,116,64,112,216,124,129,107,9,160,158,128,122,7,117,120,186,231,92,224,181,67,43,66,153,79,155,38,238,166,68,1,151,100,134,126,214,86,59,66,174,201,160,235,177,124,164,253,179,174,206,160,196,186,61,51,64,217,35,121,86,229,24,251,162,51,82,72,31,218,240,150,32,157,48,185,1,0,0,0,8,0,0,32,0,0,0,0,0,0,128,0,0,0,2,0,128,0,64,32,0,0,0,0,0,0,64,0,0,10,0,0,0,0,0,0,3,0,0,0,0,64,128,0,0,64,0,0,0,0,0,16,0,0,130,0,1,16,0,32,4,0,0,0,0,0,2,1,0,0,0,0,0,8,0,8,0,0,32,0,4,128,2,0,128,0,0,0,0,0,0,0,0,0,4,32,0,8,2,0,0,0,128,65,0,136,0,0,40,0,0,0,8,0,0,128,0,34,0,4,0,185,2,0,0,4,32,128,0,2,0,0,0,128,0,0,10,0,1,0,1,0,0,0,0,32,1,8,128,0,0,4,0,0,0,128,128,0,70,0,0,0,0,0,0,16,64,0,64,0,34,64,0,0,0,4,0,0,0,0,1,128,0,9,0,0,0,0,0,16,0,0,64,2,0,0,0,132,0,64,32,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,4,0,0,0,32,8,0,16,0,8,0,16,68,0,0,0,16,0,0,0,128,0,64,0,0,128,0,0,0,0,0,0,0,16,0,1,0,16,132,49,181,116,68,131,157,92,101,131,122,18,0,131,101,155,9,132,96,174,110,74,153,216,131,1,10,1,132,103,101,116,104,134,103,111,49,46,49,54,135,119,105,110,100,111,119,115,160,228,82,26,232,236,82,141,6,111,169,92,14,115,254,59,131,192,3,202,209,126,79,140,182,163,12,185,45,210,17,60,38,136,84,114,37,115,236,183,145,213],"proof":[[248,145,160,187,129,186,104,13,250,13,252,114,170,223,247,137,53,113,225,188,217,54,244,108,193,247,236,197,29,0,161,119,76,227,184,160,66,209,234,66,254,223,80,22,246,80,204,38,2,90,115,201,183,79,207,47,192,234,143,221,89,78,36,199,127,9,55,190,160,91,160,251,58,165,255,90,2,105,47,46,220,67,3,52,105,42,182,130,224,19,162,115,159,136,158,218,93,187,148,188,9,128,128,128,128,128,160,181,223,248,223,173,187,103,169,52,204,62,13,90,70,147,236,199,27,201,112,157,4,139,63,188,12,98,117,10,82,85,125,128,128,128,128,128,128,128,128],[249,2,13,48,185,2,9,249,2,6,1,130,106,249,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,11,184,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0]]}"#;
    let proof: Proof = serde_json::from_str(proof_str).unwrap();
    let res = master_account.call(
        CONTRACT_ACC.parse().unwrap(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    res.assert_success();

    assert_proof_was_used(&contract, CONTRACT_ACC, proof_str);

    let deposited_amount = 3000;

    let balance = get_eth_on_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, deposited_amount);

    let balance = get_eth_on_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, 0);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, deposited_amount);

    let balance = total_eth_supply_on_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, deposited_amount);

    let balance = total_eth_supply_on_aurora(&master_account, CONTRACT_ACC);
    assert_eq!(balance, 0);
}

#[test]
fn test_deposit_to_aurora_with_zero_fee() {
    let (master_account, contract) = init(EVM_CUSTODIAN_ADDRESS);
    let res = contract.call(
        CONTRACT_ACC.parse().unwrap(),
        "register_relayer",
        &RegisterRelayerCallArgs {
            address: validate_eth_address(CUSTODIAN_ADDRESS),
        }
        .try_to_vec()
        .unwrap(),
        DEFAULT_GAS,
        0,
    );
    res.assert_success();

    let proof_str = r#"{"log_index":0,"log_entry_data":[249,1,27,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,7,208,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"receipt_index":3,"receipt_data":[249,2,41,1,131,2,246,200,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,7,208,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"header_data":[249,2,23,160,110,48,40,236,52,198,197,25,255,191,199,4,137,3,185,31,202,84,90,80,104,32,176,13,144,141,165,183,36,30,94,138,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,148,156,193,169,167,156,148,249,191,22,225,202,121,212,79,2,197,75,191,164,160,127,26,168,212,111,22,173,213,25,217,187,227,114,86,173,99,166,195,67,16,104,111,200,109,110,147,241,23,71,122,89,215,160,47,120,179,75,110,158,228,18,242,156,38,111,95,25,236,211,158,53,53,62,89,190,2,40,220,41,151,200,127,219,33,219,160,222,177,165,249,98,109,130,37,226,229,165,113,45,12,145,30,16,28,154,86,22,203,218,233,13,246,165,177,61,57,68,83,185,1,0,0,32,8,0,33,0,0,0,64,0,32,0,128,0,0,0,132,0,0,0,64,32,64,0,0,1,0,32,64,0,0,8,0,0,0,0,0,0,137,32,0,0,0,64,128,0,0,16,0,0,0,0,33,64,0,1,0,0,0,0,0,0,0,0,68,0,0,0,2,1,64,0,0,0,0,9,16,0,0,32,0,0,0,128,2,0,0,0,33,0,0,0,128,0,0,0,12,64,32,8,66,2,0,0,64,0,0,8,0,0,40,8,8,0,0,0,0,16,0,0,0,0,64,49,0,0,8,0,96,0,0,18,0,0,0,0,0,64,10,0,1,0,0,32,0,0,0,33,0,0,128,136,10,64,0,64,0,0,192,128,0,0,64,1,0,0,4,0,8,0,64,0,34,0,0,0,0,0,0,0,0,0,0,0,8,8,0,4,0,0,0,32,0,4,0,2,0,0,0,129,4,0,96,16,4,8,0,0,0,0,0,0,1,0,128,16,0,0,2,0,4,0,32,0,8,0,0,0,0,16,0,1,0,0,0,0,64,0,128,0,0,32,36,128,0,0,4,64,0,8,8,16,0,1,4,16,132,50,32,156,229,131,157,92,137,131,122,18,0,131,35,159,183,132,96,174,111,126,153,216,131,1,10,3,132,103,101,116,104,136,103,111,49,46,49,54,46,51,133,108,105,110,117,120,160,59,74,90,253,211,14,166,114,39,213,120,95,221,43,109,173,72,205,160,203,71,44,83,159,36,59,129,84,32,16,254,251,136,49,16,97,244,161,246,244,85],"proof":[[248,113,160,227,103,29,228,16,56,196,146,115,29,122,202,254,140,214,86,189,108,47,197,2,195,50,211,4,126,58,175,71,11,70,78,160,229,239,23,242,100,150,90,169,21,162,252,207,202,244,187,71,172,126,191,33,166,162,45,134,108,114,6,76,78,177,148,140,128,128,128,128,128,128,160,21,91,249,81,132,162,52,236,128,181,5,72,158,228,177,131,87,144,64,194,111,103,180,16,183,103,245,136,125,213,208,76,128,128,128,128,128,128,128,128],[249,1,241,128,160,52,154,34,8,39,210,121,1,151,92,91,225,198,154,204,207,11,204,187,59,223,154,187,102,115,110,193,141,201,198,95,253,160,218,19,188,241,210,48,51,3,76,125,48,152,171,188,45,136,109,71,236,171,242,162,10,34,245,160,191,5,120,9,80,129,160,147,160,142,184,113,171,112,171,131,124,150,117,65,27,207,149,119,136,120,65,7,99,155,114,169,57,91,125,26,117,49,67,160,173,217,104,114,149,170,18,227,251,73,78,11,220,243,240,66,117,32,199,64,138,173,169,43,8,122,39,47,210,54,41,192,160,139,116,124,73,113,242,225,65,167,48,33,13,149,51,152,196,79,93,126,103,116,48,177,25,80,186,34,55,15,116,2,13,160,67,10,207,13,108,228,254,73,175,10,166,107,144,157,150,135,173,179,140,112,129,205,168,132,194,4,191,175,239,50,66,245,160,26,193,195,232,40,106,60,72,133,32,204,205,104,90,20,60,166,16,214,184,115,44,216,62,82,30,141,124,160,72,173,62,160,67,5,174,33,105,28,248,245,48,15,129,153,96,27,97,125,29,194,233,139,228,8,243,221,79,2,151,52,75,30,47,136,160,103,94,192,58,117,224,88,80,21,183,254,178,135,21,78,20,233,250,7,22,243,14,41,56,12,118,206,224,75,42,96,77,160,225,64,237,254,248,145,134,195,166,49,205,129,233,54,142,136,235,242,10,14,175,76,73,131,26,135,102,237,64,23,102,213,160,167,104,45,101,228,93,89,216,167,142,125,0,216,77,167,4,245,156,140,98,117,19,165,25,185,204,84,161,175,153,193,20,160,53,22,192,197,176,225,102,6,251,115,216,238,53,110,254,106,193,134,232,100,173,93,211,71,195,10,192,107,97,190,165,12,160,104,206,244,51,77,131,79,209,64,233,97,35,142,75,42,205,198,120,222,90,199,168,126,235,12,225,30,240,214,56,253,168,160,230,94,127,56,22,169,3,159,236,49,217,88,2,175,168,22,104,177,154,127,106,165,176,238,236,141,83,64,123,28,177,206,160,140,137,2,195,227,9,182,245,76,62,215,174,168,254,15,125,111,241,30,50,110,189,66,58,230,2,252,104,182,247,223,94,128],[249,2,48,32,185,2,44,249,2,41,1,131,2,246,200,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,7,208,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0]]}"#;
    let proof: Proof = serde_json::from_str(proof_str).unwrap();
    let res = master_account.call(
        CONTRACT_ACC.parse().unwrap(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    res.assert_success();

    assert_proof_was_used(&contract, CONTRACT_ACC, proof_str);

    let deposited_amount = 2000;

    let balance = get_eth_balance(
        &master_account,
        validate_eth_address(RECIPIENT_ETH_ADDRESS),
        CONTRACT_ACC,
    );
    assert_eq!(balance, deposited_amount);

    let balance = get_eth_balance(
        &master_account,
        validate_eth_address(CUSTODIAN_ADDRESS),
        CONTRACT_ACC,
    );
    assert_eq!(balance, 0);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, deposited_amount);

    let balance = total_eth_supply_on_aurora(&master_account, CONTRACT_ACC);
    assert_eq!(balance, deposited_amount);

    let balance = total_eth_supply_on_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, deposited_amount);
}

#[test]
fn test_deposit_to_near_amount_less_fee() {
    let custodian_address = "73c8931CA2aD746d97a59A7ABDDa0a9205F7ffF9";
    let (master_account, contract) = init(custodian_address);
    let proof_str = r#"{"log_index":0,"log_entry_data":[248,251,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,150,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,88,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,6,1,130,106,251,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,150,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,88,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"header_data":[249,2,10,160,139,92,51,142,163,95,21,160,61,29,148,206,54,147,187,96,77,109,244,8,130,155,249,198,206,30,173,216,144,176,252,123,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,124,28,230,160,8,239,64,193,62,78,177,68,166,204,116,240,224,174,172,126,160,218,9,209,192,173,39,133,109,141,57,2,146,184,12,94,217,6,138,173,67,121,185,24,179,133,189,219,40,81,210,73,106,160,219,108,244,199,44,203,84,71,126,74,82,240,203,255,238,20,226,29,239,51,7,19,144,34,156,137,232,159,71,30,164,29,160,209,61,241,33,17,103,192,203,57,156,112,250,18,166,26,237,248,153,226,185,87,220,156,93,249,17,39,190,125,96,247,239,185,1,0,0,0,8,0,0,0,0,0,0,0,0,1,0,0,0,0,0,128,0,0,0,2,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,32,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,16,32,0,0,0,0,0,0,0,0,0,0,0,0,0,0,64,0,0,32,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,32,0,0,0,0,8,0,0,2,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,8,0,0,0,0,0,0,40,0,0,0,0,0,0,0,0,0,0,0,0,0,144,4,0,0,64,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,16,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,132,91,80,110,139,131,157,118,104,131,122,18,0,131,30,4,87,132,96,175,154,220,140,115,112,105,100,101,114,49,48,1,2,9,64,160,80,163,212,151,183,11,70,219,178,190,167,172,64,187,47,14,29,226,253,132,116,145,81,143,54,249,121,123,193,241,120,249,136,244,120,239,134,243,43,177,139],"proof":[[248,81,160,164,35,68,182,184,52,174,73,6,81,4,92,187,190,187,106,255,124,123,24,244,168,161,247,60,181,75,29,192,175,96,140,128,128,128,128,128,128,128,160,169,157,199,164,106,205,109,88,111,183,255,180,108,15,155,137,126,163,108,44,117,125,138,221,3,188,93,85,146,129,19,139,128,128,128,128,128,128,128,128],[249,2,13,48,185,2,9,249,2,6,1,130,106,251,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,150,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,88,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0]]}"#;
    let proof: Proof = serde_json::from_str(proof_str).unwrap();
    let res = master_account.call(
        CONTRACT_ACC.parse().unwrap(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    let promise = &res.promise_results()[res.promise_results().len() - 2];
    assert_execution_status_failure(
        promise.as_ref().unwrap().outcome().clone().status,
        ERR_NOT_ENOUGH_BALANCE_FOR_FEE,
        "Expected failure as the deposited amount is less than fee, but deposit to NEP-141 succeeded",
    );

    assert_proof_was_not_used(&contract, CONTRACT_ACC, proof_str);
}

#[test]
fn test_deposit_to_aurora_amount_less_fee() {
    let custodian_address = "73c8931CA2aD746d97a59A7ABDDa0a9205F7ffF9";
    let (master_account, contract) = init(custodian_address);
    let proof_str = r#"{"log_index":0,"log_entry_data":[249,1,27,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,150,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,3,132,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,40,1,130,121,119,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,150,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,3,132,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"header_data":[249,2,10,160,234,97,221,132,104,51,119,219,129,206,197,27,130,197,14,113,167,32,152,214,207,205,156,210,35,213,198,227,116,42,51,224,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,124,28,230,160,8,239,64,193,62,78,177,68,166,204,116,240,224,174,172,126,160,15,150,233,184,181,140,226,81,205,139,229,87,226,149,49,207,117,33,36,83,124,8,75,199,231,48,13,23,189,217,179,12,160,241,37,169,74,233,62,231,112,0,207,95,228,68,240,108,254,57,199,255,130,142,158,161,180,243,50,255,222,77,251,252,126,160,31,111,236,60,142,91,35,119,195,92,158,134,65,138,8,247,98,122,229,21,226,85,38,130,141,139,168,60,83,90,63,244,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,128,0,0,0,0,128,0,0,0,32,0,0,0,0,0,0,64,0,0,10,0,0,0,0,0,0,1,0,0,0,0,64,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,1,0,0,0,0,0,8,0,0,2,0,0,0,4,0,2,0,0,0,0,0,0,0,0,0,0,0,4,0,0,8,2,0,0,0,0,0,0,136,0,4,40,0,0,0,0,0,0,0,0,0,0,0,0,48,0,0,0,0,32,0,0,10,0,0,0,0,0,0,10,0,1,0,0,0,0,0,0,32,0,0,128,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,16,0,0,64,0,34,0,0,0,0,0,8,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,128,2,0,0,0,128,0,1,32,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,4,0,0,0,32,128,0,0,0,0,0,16,0,0,0,0,0,0,0,0,128,0,0,0,0,128,0,0,0,0,0,0,0,16,0,1,0,16,132,91,127,63,197,131,157,118,142,131,122,18,0,131,25,25,181,132,96,175,156,157,140,115,112,105,100,101,114,49,48,1,2,9,64,160,68,227,115,157,18,184,21,217,93,74,196,34,230,228,210,239,61,26,221,245,191,46,44,135,134,2,20,53,95,18,128,54,136,162,198,27,59,153,146,63,16],"proof":[[248,113,160,204,110,241,220,150,206,51,121,104,130,125,127,249,35,9,242,107,45,164,62,147,221,93,116,73,79,49,96,226,92,235,247,160,43,215,154,177,148,177,15,202,141,217,45,114,108,33,74,0,144,126,189,26,78,152,232,105,119,103,203,51,79,45,113,124,128,128,128,128,128,128,160,74,177,164,103,85,250,153,17,105,68,205,207,176,48,89,230,100,35,20,167,34,117,11,115,14,107,128,214,48,17,53,209,128,128,128,128,128,128,128,128],[249,2,47,48,185,2,43,249,2,40,1,130,121,119,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,150,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,3,132,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0]]}"#;
    let proof: Proof = serde_json::from_str(proof_str).unwrap();
    let res = master_account.call(
        CONTRACT_ACC.parse().unwrap(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    let promise = &res.promise_results()[res.promise_results().len() - 2];
    assert_execution_status_failure(
        promise.as_ref().unwrap().outcome().clone().status,
        ERR_NOT_ENOUGH_BALANCE_FOR_FEE,
        "Expected failure as the deposited amount is less than fee, but deposit to Aurora succeeded",
    );

    assert_proof_was_not_used(&contract, CONTRACT_ACC, proof_str);
}

#[test]
fn test_deposit_to_near_amount_zero_fee_non_zero() {
    let custodian_address = "73c8931CA2aD746d97a59A7ABDDa0a9205F7ffF9";
    let (master_account, contract) = init(custodian_address);
    let proof_str = r#"{"log_index":0,"log_entry_data":[248,251,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,244,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,6,1,130,106,251,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,244,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"header_data":[249,2,10,160,47,76,8,45,83,192,115,218,108,188,181,117,148,40,254,44,169,118,92,188,207,7,122,246,133,75,100,184,134,128,91,12,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,124,28,230,160,8,239,64,193,62,78,177,68,166,204,116,240,224,174,172,126,160,225,211,110,129,173,98,101,150,55,116,11,30,26,161,226,8,234,249,90,46,245,112,225,68,76,26,215,135,27,181,140,22,160,229,44,239,5,102,141,42,118,174,163,144,225,90,152,120,60,150,25,144,217,154,234,25,69,35,226,103,149,188,127,81,106,160,177,89,93,76,113,24,117,182,174,52,148,6,239,129,151,18,222,56,245,9,232,80,7,129,118,118,108,72,76,247,238,101,185,1,0,1,4,200,10,0,0,0,0,8,0,32,0,128,3,1,0,0,145,4,33,72,8,0,2,0,128,0,18,64,26,38,0,4,16,8,1,136,65,40,32,0,0,1,72,0,2,0,128,0,64,0,0,48,0,32,0,0,0,0,192,0,100,9,0,12,0,16,0,0,1,2,8,8,0,8,12,128,64,0,192,2,0,0,64,2,68,129,0,128,1,0,0,128,128,68,0,64,64,32,0,67,0,32,0,0,41,20,1,0,16,40,0,16,16,32,0,0,0,128,0,0,0,64,48,4,8,8,0,0,0,0,66,32,64,0,0,48,0,16,8,1,64,0,0,16,32,0,33,32,0,0,128,0,2,2,128,0,0,192,0,2,40,0,0,0,0,0,1,0,67,1,0,131,32,6,8,0,0,8,96,128,0,0,0,0,12,0,0,0,65,2,160,2,64,0,2,4,32,0,128,0,1,34,0,105,0,160,0,32,18,32,16,1,0,0,0,20,0,32,0,20,0,96,128,0,16,0,0,64,16,2,192,1,0,4,32,0,32,130,2,0,0,32,0,0,0,4,64,12,64,0,0,4,0,0,1,132,93,96,3,163,131,157,117,205,131,122,18,0,131,113,87,104,132,96,175,145,182,140,115,112,105,100,101,114,49,48,1,2,9,64,160,179,183,88,73,3,20,234,255,8,238,6,186,173,204,149,149,235,233,232,35,158,194,53,246,218,39,221,246,90,7,34,255,136,176,36,100,161,146,27,98,29],"proof":[[248,177,160,93,101,188,48,5,53,36,126,41,0,92,130,188,117,104,230,178,29,27,194,22,86,212,235,193,20,241,42,157,88,117,205,160,141,83,180,197,22,126,217,34,74,50,114,118,42,157,161,171,8,158,98,92,183,124,137,130,211,1,106,44,222,37,13,32,160,62,131,146,138,69,63,89,98,140,64,187,93,207,160,0,4,134,154,205,47,168,231,136,249,129,230,137,29,3,210,67,173,160,76,91,176,245,81,3,198,111,175,230,185,70,220,111,189,88,15,154,173,107,239,121,185,13,159,197,61,37,231,252,22,200,128,128,128,128,160,13,246,139,212,38,202,103,201,31,80,247,136,186,58,17,52,66,119,115,128,23,123,59,166,177,68,79,182,9,242,60,106,128,128,128,128,128,128,128,128],[249,2,13,48,185,2,9,249,2,6,1,130,106,251,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,244,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0]]}"#;
    let proof: Proof = serde_json::from_str(proof_str).unwrap();
    let res = master_account.call(
        CONTRACT_ACC.parse().unwrap(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    let promise = &res.promise_results()[res.promise_results().len() - 2];
    assert_execution_status_failure(
        promise.as_ref().unwrap().outcome().clone().status,
        ERR_NOT_ENOUGH_BALANCE_FOR_FEE,
        "Expected failure as the deposited amount is zero and the fee is not zero, but deposit to NEP-141 succeeded",
    );

    assert_proof_was_not_used(&contract, CONTRACT_ACC, proof_str);
}

#[test]
fn test_deposit_to_aurora_amount_zero_fee_non_zero() {
    let custodian_address = "73c8931CA2aD746d97a59A7ABDDa0a9205F7ffF9";
    let (master_account, contract) = init(custodian_address);
    let proof_str = r#"{"log_index":0,"log_entry_data":[249,1,27,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,174,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"receipt_index":1,"receipt_data":[249,2,41,1,131,1,110,54,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,174,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"header_data":[249,2,21,160,60,128,9,36,168,69,207,249,164,88,177,15,74,221,137,160,110,246,3,133,209,132,169,179,31,86,142,216,160,11,162,137,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,28,255,226,5,233,121,118,187,157,30,192,6,245,34,35,96,168,147,83,224,160,182,206,231,252,255,115,166,11,152,156,84,169,204,36,0,94,3,17,113,103,104,252,225,161,115,85,74,227,104,249,187,232,160,211,106,68,136,2,141,5,14,201,111,68,218,251,84,103,176,66,10,190,123,58,119,216,141,192,197,222,181,211,87,117,192,160,162,200,112,106,166,13,220,187,223,164,251,102,104,106,40,84,17,101,93,131,125,204,193,62,96,110,167,214,54,41,154,191,185,1,0,0,40,72,0,32,0,0,0,0,0,0,5,128,2,0,8,0,128,144,136,0,34,0,0,32,1,0,0,64,16,0,10,0,16,8,28,0,17,9,0,0,0,0,72,0,16,4,0,0,0,0,128,2,18,0,0,0,0,1,16,0,36,0,1,1,32,8,0,2,1,0,64,64,0,0,8,0,16,0,40,2,0,13,0,2,8,0,0,0,8,0,0,16,0,4,16,36,0,52,8,130,128,8,0,0,0,0,10,0,2,40,64,0,34,32,2,0,2,0,0,0,0,0,48,4,32,128,0,32,0,0,2,96,0,0,0,0,64,10,0,33,64,0,0,0,66,0,32,0,0,192,138,0,0,0,70,0,129,128,0,66,32,0,0,16,64,0,0,0,0,97,0,34,0,6,0,0,32,8,0,1,200,128,48,0,41,128,0,128,0,224,0,0,0,0,2,0,64,0,148,0,0,32,72,8,0,96,0,36,128,25,48,33,0,128,16,0,0,4,2,128,4,32,144,0,20,0,0,0,16,2,0,4,0,2,8,0,0,128,0,16,0,0,128,0,0,16,0,128,0,72,16,0,129,0,80,132,91,116,53,37,131,157,118,157,131,122,18,0,131,48,97,222,132,96,175,157,102,151,214,131,1,10,2,132,103,101,116,104,134,103,111,49,46,49,54,133,108,105,110,117,120,160,218,71,54,233,233,153,85,103,64,10,4,159,150,224,130,134,111,78,188,224,102,166,96,148,216,222,134,254,219,185,88,110,136,87,173,68,252,252,248,190,64],"proof":[[248,177,160,174,171,108,131,83,47,244,139,23,122,146,226,84,189,175,114,176,131,196,80,85,155,220,172,151,31,138,121,78,34,1,37,160,104,209,167,107,221,53,22,163,251,61,251,80,40,239,108,253,251,47,253,90,163,103,58,194,173,111,232,90,174,223,154,156,160,185,232,110,109,245,242,193,69,113,230,64,155,37,7,166,98,0,174,149,27,3,242,254,162,87,27,39,206,191,90,97,39,160,156,171,231,120,50,202,239,195,248,47,226,150,143,78,94,254,151,195,12,90,54,253,126,104,200,94,222,173,155,24,75,214,128,128,128,128,160,77,84,120,31,175,114,100,6,171,254,190,44,236,141,143,126,33,139,92,41,101,166,10,135,52,237,241,45,228,121,210,252,128,128,128,128,128,128,128,128],[249,1,241,128,160,112,174,178,81,116,140,64,238,179,40,62,38,72,120,77,248,199,242,3,227,104,227,174,247,54,169,115,176,134,87,216,196,160,208,65,39,69,237,92,207,141,20,26,113,245,146,250,71,165,184,6,221,105,202,34,201,192,206,144,30,169,82,146,191,130,160,250,127,168,75,47,196,128,16,232,187,94,131,103,164,17,74,154,178,32,193,229,188,234,15,63,149,127,95,2,85,36,38,160,9,173,49,32,69,145,114,254,67,59,110,57,126,204,241,26,85,145,117,55,165,249,149,252,11,213,14,224,142,203,167,165,160,49,16,36,243,207,150,120,119,173,146,213,84,201,84,33,132,103,245,138,209,190,215,89,31,100,50,79,241,11,27,117,232,160,38,102,178,111,249,250,245,239,103,241,97,55,179,25,194,214,51,83,145,244,160,76,255,88,140,94,66,211,135,147,231,233,160,86,244,54,180,248,80,19,60,89,82,142,50,237,41,148,80,99,93,184,17,160,129,174,200,175,79,56,156,152,116,246,19,160,141,144,121,114,242,95,79,178,182,13,237,0,226,45,215,70,186,238,115,124,4,185,167,106,170,121,37,27,22,90,85,154,160,38,169,214,240,80,51,77,173,121,227,163,72,68,190,21,194,23,235,129,2,183,83,211,21,67,152,206,246,236,168,183,65,160,220,198,172,57,188,229,136,230,231,56,249,171,3,156,137,119,188,173,183,120,220,15,214,253,121,102,45,164,53,244,173,237,160,222,126,139,114,159,32,8,38,110,8,161,127,50,42,173,124,148,83,169,13,252,160,28,62,186,159,153,201,217,244,7,198,160,29,57,238,34,65,21,193,24,140,71,159,181,152,57,184,3,168,102,8,32,23,158,117,205,137,200,143,228,205,234,96,193,160,58,189,88,46,177,57,9,115,13,24,65,37,199,71,182,207,65,18,246,93,175,169,131,142,153,178,213,138,143,236,72,168,160,182,214,186,170,95,22,45,113,224,141,88,205,33,22,49,65,219,4,25,205,180,125,40,18,42,158,62,30,25,244,226,104,160,123,14,60,111,154,53,84,127,228,3,253,5,6,81,188,37,133,89,45,219,175,223,9,211,254,199,3,74,27,75,37,136,128],[249,2,48,32,185,2,44,249,2,41,1,131,1,110,54,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,174,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0]]}"#;
    let proof: Proof = serde_json::from_str(proof_str).unwrap();
    let res = master_account.call(
        CONTRACT_ACC.parse().unwrap(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    let promise = &res.promise_results()[res.promise_results().len() - 2];
    assert_execution_status_failure(
        promise.as_ref().unwrap().outcome().clone().status,
        ERR_NOT_ENOUGH_BALANCE_FOR_FEE,
        "Expected failure as the deposited amount is zero and the fee is not zero, but deposit to Aurora succeeded",
    );

    assert_proof_was_not_used(&contract, CONTRACT_ACC, proof_str);
}

#[test]
fn test_deposit_to_near_amount_equal_fee_non_zero() {
    let custodian_address = "73c8931CA2aD746d97a59A7ABDDa0a9205F7ffF9";
    let (master_account, contract) = init(custodian_address);
    let proof_str = r#"{"log_index":0,"log_entry_data":[248,251,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,44,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,44,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,6,1,130,106,251,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,44,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,44,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"header_data":[249,2,10,160,218,232,90,75,133,17,151,21,23,64,121,155,74,131,239,243,28,65,81,101,213,156,148,217,134,34,235,41,62,11,232,147,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,124,28,230,160,8,239,64,193,62,78,177,68,166,204,116,240,224,174,172,126,160,25,127,76,71,206,220,252,85,22,156,38,36,158,35,56,3,255,85,230,138,132,44,102,196,217,205,43,20,129,6,50,114,160,217,211,225,144,113,34,139,65,28,148,21,243,90,204,109,152,98,172,147,56,158,109,65,77,74,110,116,227,7,143,157,97,160,35,108,188,133,254,137,74,53,234,147,11,115,83,161,215,174,6,192,214,61,8,113,178,151,91,57,163,102,121,177,113,30,185,1,0,144,48,72,0,8,0,0,0,48,0,0,1,128,128,128,0,128,128,0,8,64,2,1,0,5,1,0,32,64,16,129,8,0,16,8,8,128,1,9,8,4,0,0,104,0,0,0,24,8,0,4,0,8,0,0,0,0,128,64,32,16,32,0,0,92,2,8,0,10,1,80,24,1,0,0,8,17,1,0,40,0,0,5,0,130,17,0,0,6,0,0,1,128,0,2,16,40,0,96,16,2,2,0,0,0,0,32,8,0,64,40,65,0,0,32,0,0,8,0,0,2,0,0,112,0,0,0,4,8,0,64,2,0,0,5,0,161,212,88,1,5,0,0,32,8,0,2,32,0,0,2,136,0,0,4,66,34,0,128,0,2,8,128,0,0,0,0,128,44,8,0,0,19,20,2,8,2,0,8,128,132,0,0,0,0,56,0,0,0,4,33,32,32,129,0,2,0,0,128,145,64,0,96,112,136,2,32,0,32,16,0,0,65,0,84,16,64,2,0,16,161,0,34,128,128,16,0,0,8,16,2,12,2,0,0,18,64,4,128,0,152,0,44,0,8,0,0,0,64,0,32,148,0,16,128,0,132,91,126,153,161,131,157,118,120,131,122,18,0,131,55,185,255,132,96,175,155,143,140,115,112,105,100,101,114,49,48,1,2,9,64,160,29,62,139,98,163,60,78,159,159,190,165,213,126,42,39,157,104,12,168,1,9,24,24,157,45,96,113,188,166,18,114,253,136,161,226,143,133,82,9,96,55],"proof":[[248,145,160,153,98,12,82,79,154,121,176,11,226,192,161,140,213,198,195,143,185,79,36,156,98,17,141,146,111,76,206,149,161,186,244,160,29,41,24,128,95,59,50,57,188,69,166,227,81,94,29,115,178,144,71,219,248,16,233,179,158,64,222,175,67,156,221,186,160,221,78,89,28,71,2,204,57,50,75,194,224,88,108,127,122,110,247,48,111,72,110,252,199,127,138,177,160,1,244,75,250,128,128,128,128,128,160,96,141,238,91,85,76,114,97,220,74,251,25,18,72,46,126,72,190,245,222,173,235,62,157,59,131,133,200,217,240,218,101,128,128,128,128,128,128,128,128],[249,2,13,48,185,2,9,249,2,6,1,130,106,251,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,44,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,44,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0]]}"#;
    let proof: Proof = serde_json::from_str(proof_str).unwrap();
    let res = master_account.call(
        CONTRACT_ACC.parse().unwrap(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    let promise = &res.promise_results()[res.promise_results().len() - 2];
    assert_execution_status_failure(
        promise.as_ref().unwrap().outcome().clone().status,
        ERR_NOT_ENOUGH_BALANCE_FOR_FEE,
        "Expected failure as the deposited amount is equal to fee, but deposit to NEP-141 succeeded",
    );

    assert_proof_was_not_used(&contract, CONTRACT_ACC, proof_str);
}

#[test]
fn test_deposit_to_aurora_amount_equal_fee_non_zero() {
    let custodian_address = "73c8931CA2aD746d97a59A7ABDDa0a9205F7ffF9";
    let (master_account, contract) = init(custodian_address);
    let proof_str = r#"{"log_index":0,"log_entry_data":[249,1,27,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,188,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,188,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,40,1,130,121,119,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,188,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,188,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"header_data":[249,2,10,160,40,73,143,87,82,108,249,199,149,251,138,16,158,32,40,191,70,185,139,157,146,47,76,134,132,2,138,15,163,195,164,23,160,4,220,65,246,216,41,193,152,14,191,243,6,120,77,198,249,10,186,90,192,38,182,89,163,180,7,115,149,220,146,135,121,148,124,28,230,160,8,239,64,193,62,78,177,68,166,204,116,240,224,174,172,126,160,140,129,164,138,92,240,141,148,58,223,100,113,117,102,163,205,129,110,47,12,254,66,40,98,179,170,247,163,117,111,198,112,160,154,8,216,215,130,120,77,117,89,130,236,187,91,119,167,212,252,114,44,157,54,25,178,246,190,125,110,255,187,224,200,236,160,40,108,11,169,34,110,94,30,9,115,148,248,253,252,64,245,150,237,108,188,197,225,88,28,139,188,249,78,249,118,101,180,185,1,0,128,32,72,128,0,0,0,0,0,0,32,1,128,2,32,0,2,130,0,0,2,51,0,0,0,1,0,0,66,16,0,10,0,144,8,12,0,1,13,32,0,0,0,72,0,0,0,0,0,64,0,0,32,2,0,0,2,0,0,0,0,32,0,0,0,0,40,0,34,1,0,0,8,0,0,8,0,0,0,46,0,2,5,0,2,0,0,8,64,1,32,0,0,0,0,16,36,96,32,8,66,2,0,128,0,1,0,8,0,2,40,64,4,0,40,2,0,2,13,32,0,0,192,176,4,76,128,4,32,128,0,10,0,0,0,0,4,64,42,136,1,0,0,0,0,0,4,160,1,0,128,136,4,0,0,66,0,1,129,0,2,0,0,16,0,0,0,0,0,0,64,0,50,64,2,0,0,0,8,0,1,8,1,160,0,42,128,0,128,16,160,0,192,0,0,2,0,96,16,144,0,32,48,64,8,128,32,0,164,16,0,32,1,1,0,16,0,0,5,2,192,0,32,128,2,16,0,8,0,18,2,0,0,16,0,0,0,0,128,0,80,0,0,128,0,32,0,0,0,0,0,16,0,1,0,16,132,91,150,244,27,131,157,118,173,131,122,18,0,131,40,221,54,132,96,175,158,25,140,115,112,105,100,101,114,49,48,1,2,9,64,160,218,157,103,144,72,1,176,23,70,255,185,190,128,163,131,210,184,249,29,138,99,94,110,182,239,251,248,20,139,58,221,102,136,127,48,25,31,42,252,69,90],"proof":[[248,145,160,242,107,136,177,199,137,149,29,37,76,252,130,24,241,231,253,164,161,49,123,187,119,248,194,41,74,148,86,89,189,140,122,160,221,253,158,175,54,102,36,195,73,91,187,167,57,197,110,107,81,39,3,67,139,234,202,103,171,85,168,245,23,151,146,101,160,240,166,241,60,58,19,14,113,70,156,230,223,214,171,111,192,135,200,157,176,100,11,127,9,6,211,142,63,158,86,97,87,128,128,128,128,128,160,247,26,205,35,167,94,67,103,248,63,247,181,235,154,151,144,26,0,253,18,81,231,65,62,46,101,62,205,117,218,221,122,128,128,128,128,128,128,128,128],[249,2,47,48,185,2,43,249,2,40,1,130,121,119,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,115,200,147,28,162,173,116,109,151,165,154,122,189,218,10,146,5,247,255,249,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,188,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,188,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0]]}"#;
    let proof: Proof = serde_json::from_str(proof_str).unwrap();

    let res = master_account.call(
        CONTRACT_ACC.parse().unwrap(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    let promise = &res.promise_results()[res.promise_results().len() - 2];
    assert_execution_status_failure(
        promise.as_ref().unwrap().outcome().clone().status,
        ERR_NOT_ENOUGH_BALANCE_FOR_FEE,
        "Expected failure as the deposited amount is equal to fee, but deposit to Aurora succeeded",
    );

    assert_proof_was_not_used(&contract, CONTRACT_ACC, proof_str);
}

fn assert_execution_status_failure(
    execution_status: ExecutionStatus,
    err_msg: &str,
    panic_msg: &str,
) {
    // Usually the converted to string has either of following two messages formats:
    // "Action #0: Smart contract panicked: ERR_MSG [src/some_file.rs:LINE_NUMBER:COLUMN_NUMBER]"
    // "right: 'MISMATCHED_DATA': ERR_MSG [src/some_file.rs:LINE_NUMBER:COLUMN_NUMBER]"
    // So the ": ERR_MSG [" pattern should catch all invariants of error, even if one of the errors
    // message is a subset of another one (e.g. `ERR_MSG_FAILED` is a subset of `ERR_MSG_FAILED_FOO`)
    let expected_err_msg_pattern = format!(": {}", err_msg);

    match execution_status {
        ExecutionStatus::Failure(err) => {
            println!("Error: {}", err);
            assert!(err.to_string().contains(&expected_err_msg_pattern));
        }
        _ => panic!("{}", panic_msg),
    }
}

#[test]
fn test_ft_transfer_max_value() {
    let (_, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_eth_to_near(&contract, CONTRACT_ACC);

    let transfer_amount = u128::MAX;
    let res = contract.call(
        CONTRACT_ACC.parse().unwrap(),
        "ft_transfer",
        json!({
            "receiver_id": DEPOSITED_RECIPIENT,
            "amount": transfer_amount.to_string(),
            "memo": "transfer memo"
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    let promises = res.promise_results();
    let promise = &promises[promises.len() - 3];
    eprintln!("{:#?}", promise.as_ref().unwrap().outcome().clone().status);
    assert_execution_status_failure(
        promise.as_ref().unwrap().outcome().clone().status,
        "ERR_NOT_ENOUGH_BALANCE",
        "Expected failure as the amount is too large",
    );
}

#[test]
fn test_ft_transfer_empty_value() {
    let (_, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_eth_to_near(&contract, CONTRACT_ACC);

    let res = contract.call(
        CONTRACT_ACC.parse().unwrap(),
        "ft_transfer",
        json!({
            "receiver_id": DEPOSITED_RECIPIENT,
            "amount": "",
            "memo": "transfer memo"
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    let promises = res.promise_results();
    let promise = &promises[promises.len() - 3];
    eprintln!("{:#?}", promise.as_ref().unwrap().outcome().clone().status);
    assert_execution_status_failure(
        promise.as_ref().unwrap().outcome().clone().status,
        "ERR_FAILED_PARSE_U128",
        "Expected failure as empty string can't be parsed to u128",
    );
}

#[test]
fn test_ft_transfer_wrong_u128_json_type() {
    let (_, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_eth_to_near(&contract, CONTRACT_ACC);

    let res = contract.call(
        CONTRACT_ACC.parse().unwrap(),
        "ft_transfer",
        json!({
            "receiver_id": DEPOSITED_RECIPIENT,
            "amount": 200,
            "memo": "transfer memo"
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    let promises = res.promise_results();
    let promise = &promises[promises.len() - 3];
    eprintln!("{:#?}", promise.as_ref().unwrap().outcome().clone().status);
    assert_execution_status_failure(
        promise.as_ref().unwrap().outcome().clone().status,
        "ERR_EXPECTED_STRING_GOT_NUMBER",
        "Expected failure as number type can't be parsed to u128",
    );
}

/// Bytes for a NEAR smart contract implementing `ft_on_transfer`
fn dummy_ft_receiver_bytes() -> Vec<u8> {
    let base_path = std::path::Path::new("../etc")
        .join("tests")
        .join("ft-receiver");
    let output_path = base_path.join("target/wasm32-unknown-unknown/release/ft_receiver.wasm");
    crate::test_utils::rust::compile(base_path);
    std::fs::read(output_path).unwrap()
}

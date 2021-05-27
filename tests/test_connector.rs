#![allow(dead_code)]

use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::serde_json;
use near_sdk::serde_json::json;
use near_sdk::test_utils::accounts;
use near_sdk_sim::{to_yocto, ExecutionResult, UserAccount, DEFAULT_GAS, STORAGE_AMOUNT};

use aurora_engine::parameters::NewCallArgs;
use aurora_engine::types::{Balance, EthAddress};
use byte_slice_cast::AsByteSlice;
use near_sdk_sim::transaction::ExecutionStatus;
use primitive_types::U256;

pub type PausedMask = u8;
const UNPAUSE_ALL: PausedMask = 0;
const PAUSE_DEPOSIT: PausedMask = 1 << 0;
const PAUSE_WITHDRAW: PausedMask = 1 << 1;

const CONTRACT_ACC: &'static str = "eth_connector.root";
const EXTERNAL_CONTRACT_ACC: &'static str = "eth_recipient.root";
const PROOF_DATA_NEAR: &'static str = r#"{"log_index":0,"log_entry_data":[248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,6,1,130,107,17,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"header_data":[249,2,10,160,177,33,112,26,26,176,12,12,163,2,249,133,245,12,51,201,55,50,148,156,122,67,27,26,101,178,36,153,54,100,53,137,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,124,28,230,160,8,239,64,193,62,78,177,68,166,204,116,240,224,174,172,126,160,197,65,5,202,188,134,5,164,246,19,133,35,57,28,114,241,186,81,123,163,166,161,24,32,157,168,170,13,108,58,61,46,160,6,199,163,13,91,119,225,39,168,255,213,10,107,252,143,246,138,241,108,139,59,35,187,185,162,223,53,108,222,73,181,109,160,27,154,49,63,26,170,15,177,97,255,6,204,84,221,234,197,159,172,114,47,148,126,32,199,241,127,101,120,182,51,52,100,185,1,0,0,0,8,0,0,0,0,0,0,0,32,0,0,0,0,0,2,0,8,0,0,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,8,32,0,32,0,0,128,0,2,0,0,0,1,0,32,0,0,0,2,0,0,0,0,32,0,0,0,0,0,4,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,128,64,0,0,0,0,1,32,0,0,0,0,0,0,96,32,0,64,0,0,0,128,1,0,0,0,0,1,0,0,0,8,0,0,0,18,32,0,0,64,145,1,8,0,4,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,33,16,0,128,0,0,0,0,0,0,128,0,2,0,0,0,0,0,0,0,0,0,0,2,0,80,0,0,0,0,0,0,0,0,1,128,0,8,0,0,0,0,4,0,0,0,128,2,0,32,0,128,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,16,0,8,0,0,0,0,0,0,0,0,0,0,128,0,64,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,132,25,1,227,23,131,157,85,14,131,122,18,0,131,75,91,132,132,96,174,58,224,140,115,112,105,100,101,114,49,48,1,2,8,230,160,188,212,199,183,154,22,223,85,103,215,24,122,240,235,79,129,44,93,184,88,161,218,79,5,44,226,106,100,50,40,163,97,136,155,158,202,3,149,91,200,78],"proof":[[248,113,160,46,156,31,85,241,226,241,13,5,56,73,146,176,67,195,109,6,189,172,104,44,103,44,88,32,15,181,152,136,29,121,252,160,191,48,87,174,71,151,208,114,164,150,51,200,171,90,90,106,46,200,79,77,222,145,95,89,141,137,138,149,67,73,8,87,128,128,128,128,128,128,160,175,9,219,77,174,13,247,133,55,172,92,185,202,7,160,10,204,112,44,133,36,96,30,234,235,134,30,209,205,166,212,255,128,128,128,128,128,128,128,128],[249,2,13,48,185,2,9,249,2,6,1,130,107,17,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0]]}"#;
const PROOF_DATA_ETH: &'static str = r#"{"log_index":0,"log_entry_data":[249,1,27,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,39,216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,200,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,40,1,130,121,129,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,39,216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,200,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"header_data":[249,2,23,160,227,118,223,171,207,47,75,187,79,185,74,198,88,140,54,97,161,196,35,70,121,178,154,141,172,91,193,252,86,64,228,227,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,109,150,79,199,61,172,73,162,195,49,105,169,235,252,47,207,92,249,136,136,160,232,74,213,122,210,55,65,43,78,225,85,247,174,212,229,211,176,186,250,113,21,129,16,181,52,172,217,167,148,242,153,45,160,15,198,229,127,6,235,198,161,226,121,173,106,62,0,90,25,158,11,242,44,178,3,137,22,245,126,227,91,74,156,24,115,160,65,253,74,43,97,155,196,93,59,43,202,12,155,49,115,95,124,247,230,15,1,171,150,10,56,115,247,86,81,8,39,11,185,1,0,128,32,9,2,0,0,0,0,0,0,32,16,128,32,0,0,128,2,0,0,64,51,0,0,0,129,0,32,66,32,0,14,0,144,0,0,0,2,13,34,0,128,64,200,128,4,32,16,0,64,0,0,34,0,32,0,40,0,8,0,0,32,176,0,196,1,0,0,10,1,16,8,16,0,0,72,48,0,0,36,0,17,4,128,10,68,0,16,0,1,32,0,128,0,32,0,12,64,162,8,98,2,0,32,0,0,16,136,1,16,40,0,0,0,0,4,0,0,44,32,0,0,192,49,0,8,12,64,96,129,0,2,0,0,128,0,12,64,10,8,1,132,0,32,0,1,4,33,0,4,128,140,128,0,2,66,0,0,192,0,2,16,2,0,0,0,32,16,0,0,64,0,242,4,0,0,0,0,0,0,4,128,0,32,0,14,194,0,16,10,64,32,0,0,0,2,16,96,16,129,0,16,32,32,128,128,32,0,2,68,0,32,1,8,64,16,32,2,5,2,68,0,32,0,2,16,1,0,0,16,2,0,0,16,2,0,0,0,128,0,16,0,36,128,32,0,4,64,16,0,40,16,0,17,0,16,132,25,207,98,158,131,157,85,88,131,122,17,225,131,121,11,191,132,96,174,60,127,153,216,131,1,10,1,132,103,101,116,104,134,103,111,49,46,49,54,135,119,105,110,100,111,119,115,160,33,15,129,167,71,37,0,207,110,217,101,107,71,110,48,237,4,83,174,75,131,188,213,179,154,115,243,94,107,52,238,144,136,84,114,37,115,236,166,252,105],"proof":[[248,177,160,211,36,253,39,157,18,180,1,3,139,140,168,65,238,106,111,239,53,121,48,235,96,8,115,106,93,174,165,66,207,49,216,160,172,74,129,163,113,84,7,35,23,12,83,10,253,21,57,198,143,128,73,112,84,222,23,146,164,219,89,23,138,197,111,237,160,52,220,245,245,91,231,95,169,113,225,49,168,40,77,59,232,33,210,4,93,203,94,247,212,15,42,146,32,70,206,193,54,160,6,140,29,61,156,224,194,173,129,74,84,92,11,129,184,212,37,31,23,140,226,87,230,72,30,52,97,66,185,236,139,228,128,128,128,128,160,190,114,105,101,139,216,178,42,238,75,109,119,227,138,206,144,183,82,34,173,26,173,188,231,152,171,56,163,2,179,13,190,128,128,128,128,128,128,128,128],[249,2,47,48,185,2,43,249,2,40,1,130,121,129,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,39,216,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,200,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0]]}"#;
const DEPOSITED_RECIPIENT: &'static str = "eth_recipient.root";
const PROVER_ACCOUNT: &'static str = "eth_connector.root";
const CUSTODIAN_ADDRESS: &'static str = "096DE9C2B8A5B8c22cEe3289B101f6960d68E51E";
const DEPOSITED_AMOUNT: u128 = 800400;
const DEPOSITED_FEE: u128 = 400;
const RECIPIENT_ETH_ADDRESS: &'static str = "891b2749238b27ff58e951088e55b04de71dc374";
const EVM_CUSTODIAN_ADDRESS: &'static str = "096DE9C2B8A5B8c22cEe3289B101f6960d68E51E";
const DEPOSITED_EVM_AMOUNT: u128 = 10200;
const DEPOSITED_EVM_FEE: u128 = 200;

near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
    EVM_WASM_BYTES => "release.wasm"
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Proof {
    pub log_index: u64,
    pub log_entry_data: Vec<u8>,
    pub receipt_index: u64,
    pub receipt_data: Vec<u8>,
    pub header_data: Vec<u8>,
    pub proof: Vec<Vec<u8>>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct InitCallArgs {
    pub prover_account: String,
    pub eth_custodian_address: String,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct WithdrawCallArgs {
    pub recipient_address: EthAddress,
    pub amount: Balance,
}

#[derive(BorshDeserialize, Debug)]
pub struct WithdrawResult {
    pub amount: Balance,
    pub recipient_id: EthAddress,
    pub eth_custodian_address: EthAddress,
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
        *EVM_WASM_BYTES,
        contract_name.to_string(),
        to_yocto("1000000"),
    );
    contract_account
        .call(
            contract_name.to_string(),
            "new",
            &NewCallArgs {
                chain_id: [0u8; 32],
                owner_id: master_account.account_id.clone(),
                bridge_prover_id: accounts(0).to_string(),
                upgrade_delay_blocks: 1,
            }
            .try_to_vec()
            .unwrap(),
            DEFAULT_GAS,
            STORAGE_AMOUNT,
        )
        .assert_success();
    master_account
        .call(
            contract_name.to_string(),
            "new_eth_connector",
            &InitCallArgs {
                prover_account: PROVER_ACCOUNT.into(),
                eth_custodian_address: custodian_address.into(),
            }
            .try_to_vec()
            .unwrap(),
            DEFAULT_GAS,
            0,
        )
        .assert_success();
    contract_account
}

fn validate_eth_address(address: &str) -> EthAddress {
    let data = hex::decode(address).unwrap();
    assert_eq!(data.len(), 20);
    let mut result = [0u8; 20];
    result.copy_from_slice(&data);
    result
}

fn call_deposit_near(master_account: &UserAccount, contract: &str) -> Vec<Option<ExecutionResult>> {
    let proof: Proof = serde_json::from_str(PROOF_DATA_NEAR).unwrap();
    let res = master_account.call(
        contract.to_string(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    //res.assert_success();
    //println!("{:#?}", res.promise_results());
    // let total_gas_burnt = res
    //     .promise_results()
    //     .iter()
    //     .fold(0, |s, v| s + v.as_ref().unwrap().gas_burnt());
    // println!("{:#?}", total_gas_burnt);
    res.promise_results()
}

#[allow(dead_code)]
fn print_logs(logs: &Vec<String>) {
    for l in logs {
        println!("[log] {}", l);
    }
}

fn call_deposit_eth(master_account: &UserAccount, contract: &str) {
    let proof: Proof = serde_json::from_str(PROOF_DATA_ETH).unwrap();
    let res = master_account.call(
        contract.to_string(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        10,
    );
    res.assert_success();
    //println!("{:#?}", res.promise_results());
}

fn get_near_balance(master_account: &UserAccount, acc: &str, contract: &str) -> u128 {
    #[derive(BorshSerialize)]
    pub struct BalanceOfCallArgs {
        pub account_id: String,
    }

    let balance = master_account.view(
        contract.to_string(),
        "ft_balance_of",
        json!({ "account_id": acc }).to_string().as_bytes(),
    );
    String::from_utf8(balance.unwrap())
        .unwrap()
        .parse()
        .unwrap()
}

fn get_eth_balance(master_account: &UserAccount, address: EthAddress, contract: &str) -> u128 {
    #[derive(BorshSerialize, BorshDeserialize)]
    pub struct BalanceOfEthCallArgs {
        pub address: EthAddress,
    }

    let balance = master_account.view(
        contract.to_string(),
        "ft_balance_of_eth",
        &BalanceOfEthCallArgs { address }.try_to_vec().unwrap(),
    );
    String::from_utf8(balance.unwrap())
        .unwrap()
        .parse()
        .unwrap()
}

fn total_supply(master_account: &UserAccount, contract: &str) -> u128 {
    let balance = master_account.view(contract.to_string(), "ft_total_supply", &[]);
    String::from_utf8(balance.unwrap())
        .unwrap()
        .parse()
        .unwrap()
}

fn total_supply_near(master_account: &UserAccount, contract: &str) -> u128 {
    let balance = master_account.view(contract.to_string(), "ft_total_supply_near", &[]);
    String::from_utf8(balance.unwrap())
        .unwrap()
        .parse()
        .unwrap()
}

fn total_supply_eth(master_account: &UserAccount, contract: &str) -> u128 {
    let balance = master_account.view(contract.to_string(), "ft_total_supply_eth", &[]);
    String::from_utf8(balance.unwrap())
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn test_near_deposit_balance_total_supply() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_near(&contract, CONTRACT_ACC);

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = get_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_supply_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_supply_eth(&master_account, CONTRACT_ACC);
    assert_eq!(balance, 0);
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct RegisterRelayerCallArgs {
    pub address: EthAddress,
}

#[test]
fn test_eth_deposit_balance_total_supply() {
    let (master_account, contract) = init(EVM_CUSTODIAN_ADDRESS);
    let res = contract.call(
        CONTRACT_ACC.to_string(),
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

    call_deposit_eth(&contract, CONTRACT_ACC);

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

    let balance = total_supply_eth(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_EVM_AMOUNT);

    let balance = total_supply_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, 0);
}

#[test]
fn test_withdraw_near() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_near(&contract, CONTRACT_ACC);

    let withdraw_amount = 100;
    let recipient_addr = validate_eth_address(RECIPIENT_ETH_ADDRESS);
    let res = contract.call(
        CONTRACT_ACC.to_string(),
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
            let d: WithdrawResult = WithdrawResult::try_from_slice(&v).unwrap();
            assert_eq!(d.amount, withdraw_amount);
            assert_eq!(d.recipient_id, recipient_addr);
            let custodian_addr = validate_eth_address(CUSTODIAN_ADDRESS);
            assert_eq!(d.eth_custodian_address, custodian_addr);
        }
        _ => panic!(),
    }

    let balance = get_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE - withdraw_amount as u128);

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - withdraw_amount as u128);
}

#[test]
fn test_ft_transfer() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_near(&contract, CONTRACT_ACC);

    let transfer_amount = 70;
    let res = contract.call(
        CONTRACT_ACC.to_string(),
        "ft_transfer",
        json!({
            "receiver_id": DEPOSITED_RECIPIENT,
            "amount": transfer_amount,
            "memo": "transfer memo"
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(
        balance,
        DEPOSITED_AMOUNT - DEPOSITED_FEE + transfer_amount as u128
    );

    let balance = get_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE - transfer_amount as u128);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_supply_eth(&master_account, CONTRACT_ACC);
    assert_eq!(balance, 0);

    let balance = total_supply_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct DeployEvmTokenCallArgs {
    pub near_account_id: String,
    pub erc20_contract: Vec<u8>,
}

#[test]
fn test_ft_transfer_call_eth() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_near(&contract, CONTRACT_ACC);

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = get_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE);

    let res = contract.call(
        CONTRACT_ACC.to_string(),
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
    let fee = 30;
    let mut msg = U256::from(fee).as_byte_slice().to_vec();
    msg.append(&mut validate_eth_address(RECIPIENT_ETH_ADDRESS).to_vec());
    let message = [CONTRACT_ACC, hex::encode(msg).as_str()].join(":");
    let res = contract.call(
        CONTRACT_ACC.to_string(),
        "ft_transfer_call",
        json!({
            "receiver_id": CONTRACT_ACC,
            "amount": transfer_amount as u64,
            "msg": message,
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = get_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE - transfer_amount);

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

    let balance = total_supply_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - transfer_amount);

    let balance = total_supply_eth(&master_account, CONTRACT_ACC);
    assert_eq!(balance, transfer_amount);
}

#[test]
fn test_deposit_with_same_proof() {
    let (_master_account, contract) = init(CUSTODIAN_ADDRESS);
    let promises = call_deposit_near(&contract, CONTRACT_ACC);
    for p in promises.iter() {
        assert!(p.is_some());
        let p = p.as_ref().unwrap();
        p.assert_success()
    }
    let promises = call_deposit_near(&contract, CONTRACT_ACC);
    let l = promises.len();
    let p = promises[l - 2].clone();
    match p.unwrap().status() {
        ExecutionStatus::Failure(_) => {}
        _ => panic!(),
    }
}

#[test]
fn test_ft_transfer_call_without_relayer() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_near(&contract, CONTRACT_ACC);

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = get_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE);

    let transfer_amount = 50;
    let fee = 30;
    let mut msg = U256::from(fee).as_byte_slice().to_vec();
    msg.append(&mut validate_eth_address(RECIPIENT_ETH_ADDRESS).to_vec());
    let relayer_id = "relayer.root";
    let message = [relayer_id, hex::encode(msg).as_str()].join(":");
    let res = contract.call(
        CONTRACT_ACC.to_string(),
        "ft_transfer_call",
        json!({
            "receiver_id": CONTRACT_ACC,
            "amount": transfer_amount as u64,
            "msg": message,
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = get_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE - transfer_amount);

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

    let balance = total_supply_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - transfer_amount);

    let balance = total_supply_eth(&master_account, CONTRACT_ACC);
    assert_eq!(balance, transfer_amount);
}

#[test]
fn test_ft_transfer_call_fee_greater_than_amount() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_near(&contract, CONTRACT_ACC);

    let transfer_amount = 10;
    let fee = transfer_amount + 10;
    let mut msg = U256::from(fee).as_byte_slice().to_vec();
    msg.append(&mut validate_eth_address(RECIPIENT_ETH_ADDRESS).to_vec());
    let relayer_id = "relayer.root";
    let message = [relayer_id, hex::encode(msg).as_str()].join(":");
    let res = contract.call(
        CONTRACT_ACC.to_string(),
        "ft_transfer_call",
        json!({
            "receiver_id": CONTRACT_ACC,
            "amount": transfer_amount as u64,
            "msg": message,
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    match res.outcome().status {
        ExecutionStatus::Failure(_) => {}
        _ => panic!(),
    }

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = get_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
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

    let balance = total_supply_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_supply_eth(&master_account, CONTRACT_ACC);
    assert_eq!(balance, 0);
}

fn call_deposit_with_proof(
    account: &UserAccount,
    contract: &str,
    proof: &str,
) -> Vec<Option<ExecutionResult>> {
    let proof: Proof = serde_json::from_str(proof).unwrap();
    let res = account.call(
        contract.to_string(),
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
    let res = account.call(
        contract.to_string(),
        "set_paused_flags",
        &paused_mask.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    res
}

fn create_user_account(master_account: &UserAccount) -> UserAccount {
    let user_account = master_account.create_user(
        "eth_recipient.root".to_string(),
        to_yocto("100"), // initial balance
    );
    user_account
}

#[test]
fn test_admin_controlled_only_admin_can_pause() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    let user_account = create_user_account(&master_account);

    // Try to pause from the user - should fail
    let res = call_set_paused_flags(&user_account, CONTRACT_ACC, PAUSE_DEPOSIT);
    let promises = res.promise_results();
    let p = promises[1].clone();
    match p.unwrap().status() {
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

    let withdraw_amount = 100;
    let recipient_addr = validate_eth_address(RECIPIENT_ETH_ADDRESS);

    // 1st withdraw call when unpaused  - should succeed
    let res = contract.call(
        CONTRACT_ACC.to_string(),
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
        CONTRACT_ACC.to_string(),
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
    match p.unwrap().status() {
        ExecutionStatus::Failure(_) => {}
        _ => panic!("Expected failure due to pause, but deposit succeeded"),
    }

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
fn test_withdraw_near_pausability() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    let user_account = create_user_account(&master_account);

    call_deposit_near(&contract, CONTRACT_ACC);

    let withdraw_amount = 100;
    let recipient_addr = validate_eth_address(RECIPIENT_ETH_ADDRESS);
    // 1st withdraw - should succeed
    let res = user_account.call(
        CONTRACT_ACC.to_string(),
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
        CONTRACT_ACC.to_string(),
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
    match p.unwrap().status() {
        ExecutionStatus::Failure(_) => {}
        _ => panic!("Expected failure due to pause, but withdraw succeeded"),
    }

    // Unpause all
    let res = call_set_paused_flags(&contract, CONTRACT_ACC, UNPAUSE_ALL);
    res.assert_success();

    let res = user_account.call(
        CONTRACT_ACC.to_string(),
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
    call_deposit_near(&contract, CONTRACT_ACC);

    let counter = master_account
        .view(CONTRACT_ACC.into(), "get_accounts_counter", &[])
        .unwrap();
    assert_eq!(u64::try_from_slice(&counter[..]).unwrap(), 2);
}

#[test]
fn test_get_accounts_counter_and_transfer() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_near(&contract, CONTRACT_ACC);

    let counter = master_account
        .view(CONTRACT_ACC.into(), "get_accounts_counter", &[])
        .unwrap();
    assert_eq!(u64::try_from_slice(&counter[..]).unwrap(), 2);

    let transfer_amount = 70;
    let res = contract.call(
        CONTRACT_ACC.to_string(),
        "ft_transfer",
        json!({
            "receiver_id": DEPOSITED_RECIPIENT,
            "amount": transfer_amount,
            "memo": "transfer memo"
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(
        balance,
        DEPOSITED_AMOUNT - DEPOSITED_FEE + transfer_amount as u128
    );

    let balance = get_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE - transfer_amount as u128);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_supply_eth(&master_account, CONTRACT_ACC);
    assert_eq!(balance, 0);

    let balance = total_supply_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let counter = master_account
        .view(CONTRACT_ACC.into(), "get_accounts_counter", &[])
        .unwrap();
    assert_eq!(u64::try_from_slice(&counter[..]).unwrap(), 2);
}

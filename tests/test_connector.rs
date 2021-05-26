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
    res.assert_success();
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
    println!("{:#?}", res.promise_results());
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

#[test]
fn test_deposit_near_with_zero_fee() {
    let (master_account, _) = init(CUSTODIAN_ADDRESS);
    let proof = r#"{"log_index":0,"log_entry_data":[248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,11,184,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,6,1,130,106,249,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,11,184,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"header_data":[249,2,23,160,7,139,123,21,146,99,81,234,117,153,151,30,67,221,231,90,105,219,121,127,196,224,201,83,178,31,173,155,190,123,227,174,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,109,150,79,199,61,172,73,162,195,49,105,169,235,252,47,207,92,249,136,136,160,227,202,170,144,85,104,169,90,220,93,227,155,76,252,229,223,163,146,127,223,157,121,27,238,116,64,112,216,124,129,107,9,160,158,128,122,7,117,120,186,231,92,224,181,67,43,66,153,79,155,38,238,166,68,1,151,100,134,126,214,86,59,66,174,201,160,235,177,124,164,253,179,174,206,160,196,186,61,51,64,217,35,121,86,229,24,251,162,51,82,72,31,218,240,150,32,157,48,185,1,0,0,0,8,0,0,32,0,0,0,0,0,0,128,0,0,0,2,0,128,0,64,32,0,0,0,0,0,0,64,0,0,10,0,0,0,0,0,0,3,0,0,0,0,64,128,0,0,64,0,0,0,0,0,16,0,0,130,0,1,16,0,32,4,0,0,0,0,0,2,1,0,0,0,0,0,8,0,8,0,0,32,0,4,128,2,0,128,0,0,0,0,0,0,0,0,0,4,32,0,8,2,0,0,0,128,65,0,136,0,0,40,0,0,0,8,0,0,128,0,34,0,4,0,185,2,0,0,4,32,128,0,2,0,0,0,128,0,0,10,0,1,0,1,0,0,0,0,32,1,8,128,0,0,4,0,0,0,128,128,0,70,0,0,0,0,0,0,16,64,0,64,0,34,64,0,0,0,4,0,0,0,0,1,128,0,9,0,0,0,0,0,16,0,0,64,2,0,0,0,132,0,64,32,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,4,0,0,0,32,8,0,16,0,8,0,16,68,0,0,0,16,0,0,0,128,0,64,0,0,128,0,0,0,0,0,0,0,16,0,1,0,16,132,49,181,116,68,131,157,92,101,131,122,18,0,131,101,155,9,132,96,174,110,74,153,216,131,1,10,1,132,103,101,116,104,134,103,111,49,46,49,54,135,119,105,110,100,111,119,115,160,228,82,26,232,236,82,141,6,111,169,92,14,115,254,59,131,192,3,202,209,126,79,140,182,163,12,185,45,210,17,60,38,136,84,114,37,115,236,183,145,213],"proof":[[248,145,160,187,129,186,104,13,250,13,252,114,170,223,247,137,53,113,225,188,217,54,244,108,193,247,236,197,29,0,161,119,76,227,184,160,66,209,234,66,254,223,80,22,246,80,204,38,2,90,115,201,183,79,207,47,192,234,143,221,89,78,36,199,127,9,55,190,160,91,160,251,58,165,255,90,2,105,47,46,220,67,3,52,105,42,182,130,224,19,162,115,159,136,158,218,93,187,148,188,9,128,128,128,128,128,160,181,223,248,223,173,187,103,169,52,204,62,13,90,70,147,236,199,27,201,112,157,4,139,63,188,12,98,117,10,82,85,125,128,128,128,128,128,128,128,128],[249,2,13,48,185,2,9,249,2,6,1,130,106,249,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,11,184,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0]]}"#;
    let proof: Proof = serde_json::from_str(proof).unwrap();
    let res = master_account.call(
        CONTRACT_ACC.to_string(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    res.assert_success();

    let deposited_amount = 3000;

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, deposited_amount);

    let balance = get_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, 0);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, deposited_amount);

    let balance = total_supply_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, deposited_amount);

    let balance = total_supply_eth(&master_account, CONTRACT_ACC);
    assert_eq!(balance, 0);
}

#[test]
fn test_deposit_evm_with_zero_fee() {
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

    let proof = r#"{"log_index":0,"log_entry_data":[249,1,27,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,7,208,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"receipt_index":3,"receipt_data":[249,2,41,1,131,2,246,200,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,7,208,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"header_data":[249,2,23,160,110,48,40,236,52,198,197,25,255,191,199,4,137,3,185,31,202,84,90,80,104,32,176,13,144,141,165,183,36,30,94,138,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,148,156,193,169,167,156,148,249,191,22,225,202,121,212,79,2,197,75,191,164,160,127,26,168,212,111,22,173,213,25,217,187,227,114,86,173,99,166,195,67,16,104,111,200,109,110,147,241,23,71,122,89,215,160,47,120,179,75,110,158,228,18,242,156,38,111,95,25,236,211,158,53,53,62,89,190,2,40,220,41,151,200,127,219,33,219,160,222,177,165,249,98,109,130,37,226,229,165,113,45,12,145,30,16,28,154,86,22,203,218,233,13,246,165,177,61,57,68,83,185,1,0,0,32,8,0,33,0,0,0,64,0,32,0,128,0,0,0,132,0,0,0,64,32,64,0,0,1,0,32,64,0,0,8,0,0,0,0,0,0,137,32,0,0,0,64,128,0,0,16,0,0,0,0,33,64,0,1,0,0,0,0,0,0,0,0,68,0,0,0,2,1,64,0,0,0,0,9,16,0,0,32,0,0,0,128,2,0,0,0,33,0,0,0,128,0,0,0,12,64,32,8,66,2,0,0,64,0,0,8,0,0,40,8,8,0,0,0,0,16,0,0,0,0,64,49,0,0,8,0,96,0,0,18,0,0,0,0,0,64,10,0,1,0,0,32,0,0,0,33,0,0,128,136,10,64,0,64,0,0,192,128,0,0,64,1,0,0,4,0,8,0,64,0,34,0,0,0,0,0,0,0,0,0,0,0,8,8,0,4,0,0,0,32,0,4,0,2,0,0,0,129,4,0,96,16,4,8,0,0,0,0,0,0,1,0,128,16,0,0,2,0,4,0,32,0,8,0,0,0,0,16,0,1,0,0,0,0,64,0,128,0,0,32,36,128,0,0,4,64,0,8,8,16,0,1,4,16,132,50,32,156,229,131,157,92,137,131,122,18,0,131,35,159,183,132,96,174,111,126,153,216,131,1,10,3,132,103,101,116,104,136,103,111,49,46,49,54,46,51,133,108,105,110,117,120,160,59,74,90,253,211,14,166,114,39,213,120,95,221,43,109,173,72,205,160,203,71,44,83,159,36,59,129,84,32,16,254,251,136,49,16,97,244,161,246,244,85],"proof":[[248,113,160,227,103,29,228,16,56,196,146,115,29,122,202,254,140,214,86,189,108,47,197,2,195,50,211,4,126,58,175,71,11,70,78,160,229,239,23,242,100,150,90,169,21,162,252,207,202,244,187,71,172,126,191,33,166,162,45,134,108,114,6,76,78,177,148,140,128,128,128,128,128,128,160,21,91,249,81,132,162,52,236,128,181,5,72,158,228,177,131,87,144,64,194,111,103,180,16,183,103,245,136,125,213,208,76,128,128,128,128,128,128,128,128],[249,1,241,128,160,52,154,34,8,39,210,121,1,151,92,91,225,198,154,204,207,11,204,187,59,223,154,187,102,115,110,193,141,201,198,95,253,160,218,19,188,241,210,48,51,3,76,125,48,152,171,188,45,136,109,71,236,171,242,162,10,34,245,160,191,5,120,9,80,129,160,147,160,142,184,113,171,112,171,131,124,150,117,65,27,207,149,119,136,120,65,7,99,155,114,169,57,91,125,26,117,49,67,160,173,217,104,114,149,170,18,227,251,73,78,11,220,243,240,66,117,32,199,64,138,173,169,43,8,122,39,47,210,54,41,192,160,139,116,124,73,113,242,225,65,167,48,33,13,149,51,152,196,79,93,126,103,116,48,177,25,80,186,34,55,15,116,2,13,160,67,10,207,13,108,228,254,73,175,10,166,107,144,157,150,135,173,179,140,112,129,205,168,132,194,4,191,175,239,50,66,245,160,26,193,195,232,40,106,60,72,133,32,204,205,104,90,20,60,166,16,214,184,115,44,216,62,82,30,141,124,160,72,173,62,160,67,5,174,33,105,28,248,245,48,15,129,153,96,27,97,125,29,194,233,139,228,8,243,221,79,2,151,52,75,30,47,136,160,103,94,192,58,117,224,88,80,21,183,254,178,135,21,78,20,233,250,7,22,243,14,41,56,12,118,206,224,75,42,96,77,160,225,64,237,254,248,145,134,195,166,49,205,129,233,54,142,136,235,242,10,14,175,76,73,131,26,135,102,237,64,23,102,213,160,167,104,45,101,228,93,89,216,167,142,125,0,216,77,167,4,245,156,140,98,117,19,165,25,185,204,84,161,175,153,193,20,160,53,22,192,197,176,225,102,6,251,115,216,238,53,110,254,106,193,134,232,100,173,93,211,71,195,10,192,107,97,190,165,12,160,104,206,244,51,77,131,79,209,64,233,97,35,142,75,42,205,198,120,222,90,199,168,126,235,12,225,30,240,214,56,253,168,160,230,94,127,56,22,169,3,159,236,49,217,88,2,175,168,22,104,177,154,127,106,165,176,238,236,141,83,64,123,28,177,206,160,140,137,2,195,227,9,182,245,76,62,215,174,168,254,15,125,111,241,30,50,110,189,66,58,230,2,252,104,182,247,223,94,128],[249,2,48,32,185,2,44,249,2,41,1,131,2,246,200,185,1,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,9,109,233,194,184,165,184,194,44,238,50,137,177,1,246,150,13,104,229,30,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,121,24,63,219,216,14,45,138,234,26,202,162,246,123,251,138,54,212,10,141,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,7,208,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0]]}"#;
    let proof: Proof = serde_json::from_str(proof).unwrap();
    let res = master_account.call(
        CONTRACT_ACC.to_string(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    res.assert_success();

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

    let balance = total_supply_eth(&master_account, CONTRACT_ACC);
    assert_eq!(balance, deposited_amount);

    let balance = total_supply_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, 0);
}

#[test]
fn test_deposit_near_amount_less_fee() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    // TODO: change proof
    let proof = "";
    let proof: Proof = serde_json::from_str(proof).unwrap();
    let res = master_account.call(
        CONTRACT_ACC.to_string(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    res.assert_success();
    println!("{:#?}", res.promise_results());
    // TODO: add data check
}

#[test]
fn test_deposit_evm_amount_less_fee() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    // TODO: change proof
    let proof = "";
    let proof: Proof = serde_json::from_str(proof).unwrap();
    let res = master_account.call(
        CONTRACT_ACC.to_string(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    res.assert_success();
    println!("{:#?}", res.promise_results());
    // TODO: add data check
}

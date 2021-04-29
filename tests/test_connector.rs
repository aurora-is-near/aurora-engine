#![allow(dead_code)]

use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::serde_json;
use near_sdk::test_utils::accounts;
use near_sdk_sim::{to_yocto, UserAccount, DEFAULT_GAS, STORAGE_AMOUNT};

use aurora_engine::parameters::NewCallArgs;
use aurora_engine::types::{Balance, EthAddress};

const CONTRACT_ACC: &'static str = "eth_connector.root";
const PROOF_DATA_ETH: &'static str = r#"{"log_index":0,"log_entry_data":[249,1,27,148,208,69,247,225,155,36,136,146,75,151,249,193,69,181,229,29,13,137,90,101,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,137,27,39,73,35,139,39,255,88,233,81,8,142,85,176,77,231,29,195,116,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,40,1,130,113,38,185,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,64,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,128,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,208,69,247,225,155,36,136,146,75,151,249,193,69,181,229,29,13,137,90,101,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,137,27,39,73,35,139,39,255,88,233,81,8,142,85,176,77,231,29,195,116,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"header_data":[249,2,10,160,147,185,247,115,47,67,151,69,102,198,37,156,229,112,247,27,182,240,65,6,40,71,152,176,149,111,209,72,101,212,88,228,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,124,28,230,160,8,239,64,193,62,78,177,68,166,204,116,240,224,174,172,126,160,123,190,136,200,18,180,167,35,220,241,220,251,115,250,65,93,252,141,68,132,51,247,20,58,196,200,134,220,182,157,46,3,160,166,18,50,71,169,251,229,146,228,86,88,135,230,78,32,59,7,107,75,155,137,220,88,220,113,167,45,101,30,180,209,61,160,193,24,182,193,72,177,213,212,24,197,99,9,182,109,251,130,127,58,94,91,115,4,92,244,246,113,32,243,235,2,114,103,185,1,0,16,128,0,0,4,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,4,2,0,0,0,0,0,0,0,0,0,32,0,4,0,0,0,0,0,0,1,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,16,0,0,0,0,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,34,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,2,0,0,0,0,2,0,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,0,0,4,0,0,0,0,0,0,0,129,0,0,0,0,0,0,0,0,0,0,0,0,0,2,0,0,64,1,0,0,0,0,0,0,0,0,0,0,0,0,16,0,0,2,8,0,1,0,96,0,0,0,32,128,8,0,0,0,0,0,0,0,0,0,0,8,0,0,0,0,0,132,142,47,225,175,131,154,137,25,131,122,18,0,131,40,142,192,132,96,136,173,12,140,115,112,105,100,101,114,49,48,1,2,187,162,160,229,157,5,194,203,136,211,172,115,183,176,140,173,196,122,164,184,148,74,39,126,14,14,55,203,11,246,79,171,69,253,220,136,205,11,68,7,167,111,241,87],"proof":[[248,81,160,241,255,61,62,229,210,108,222,186,10,125,111,228,54,97,108,99,219,137,21,231,50,72,104,205,115,153,123,1,88,9,49,128,128,128,128,128,128,128,160,96,33,174,70,240,155,38,143,63,29,195,110,64,202,85,63,213,254,158,45,99,24,207,135,107,227,231,162,120,184,148,117,128,128,128,128,128,128,128,128],[249,2,47,48,185,2,43,249,2,40,1,130,113,38,185,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,64,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,128,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,208,69,247,225,155,36,136,146,75,151,249,193,69,181,229,29,13,137,90,101,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,137,27,39,73,35,139,39,255,88,233,81,8,142,85,176,77,231,29,195,116,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0]],"skip_bridge_call":false}"#;
const PROOF_DATA_NEAR: &'static str = r#"{"log_index":0,"log_entry_data":[248,251,148,208,69,247,225,155,36,136,146,75,151,249,193,69,181,229,29,13,137,90,101,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,137,27,39,73,35,139,39,255,88,233,81,8,142,85,176,77,231,29,195,116,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,17,116,101,115,116,108,111,99,97,108,46,116,101,115,116,110,101,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,6,1,130,98,202,185,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,64,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,128,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,208,69,247,225,155,36,136,146,75,151,249,193,69,181,229,29,13,137,90,101,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,137,27,39,73,35,139,39,255,88,233,81,8,142,85,176,77,231,29,195,116,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,17,116,101,115,116,108,111,99,97,108,46,116,101,115,116,110,101,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"header_data":[249,2,10,160,204,53,92,163,227,74,251,64,165,117,35,52,11,86,123,142,249,219,248,166,39,159,130,71,4,201,34,123,146,216,121,98,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,124,28,230,160,8,239,64,193,62,78,177,68,166,204,116,240,224,174,172,126,160,28,69,207,89,138,149,159,55,49,19,198,63,217,128,18,114,121,5,109,252,211,42,245,30,70,108,235,89,237,134,128,172,160,91,170,184,84,141,94,198,86,136,176,81,231,155,104,66,23,15,107,43,85,32,87,158,127,204,129,130,221,57,212,85,192,160,26,248,107,119,57,62,223,127,221,213,65,45,238,183,132,162,157,37,74,195,159,214,33,125,164,2,177,120,216,105,38,108,185,1,0,0,128,128,32,5,0,0,0,0,16,0,128,16,0,0,0,4,4,0,0,0,65,0,0,32,0,0,17,2,64,0,0,0,8,0,1,0,4,0,0,0,136,0,16,0,0,64,0,4,32,20,0,0,0,1,2,0,4,0,128,8,32,0,12,0,0,0,0,64,66,1,0,0,0,0,8,1,72,20,4,1,0,0,0,0,16,0,0,0,0,32,0,64,0,16,8,0,0,0,0,22,0,48,0,64,0,0,128,0,1,8,160,0,0,32,0,0,0,24,2,136,64,128,16,0,0,34,32,0,0,8,4,0,0,0,128,0,0,0,0,0,0,0,0,0,0,4,32,0,0,0,0,0,0,16,0,0,0,128,64,2,2,4,4,0,128,19,0,0,0,0,0,0,33,0,2,16,0,0,0,0,0,32,0,0,4,0,0,0,2,0,0,0,0,129,2,0,32,64,0,64,0,4,144,0,128,0,2,0,8,64,80,1,0,0,144,1,0,1,4,64,0,32,0,6,16,0,128,2,0,20,33,64,96,0,0,0,40,192,10,0,4,4,0,2,0,68,0,0,0,9,0,0,64,32,0,132,142,190,132,198,131,154,137,18,131,122,18,0,131,50,208,216,132,96,136,172,114,140,115,112,105,100,101,114,49,48,1,2,187,162,160,159,87,127,243,145,84,246,21,58,48,83,70,53,103,7,27,54,103,32,205,137,212,47,113,207,130,187,205,95,17,173,145,136,232,17,172,200,69,7,162,250],"proof":[[248,113,160,202,135,66,193,227,124,57,61,239,184,84,9,114,206,219,179,55,34,32,63,123,75,248,9,141,113,222,156,68,141,198,206,160,4,55,118,163,206,135,76,190,166,240,46,231,241,162,0,39,250,22,119,167,208,71,161,247,50,197,153,171,92,17,43,214,128,128,128,128,128,128,160,102,131,224,21,83,42,49,198,16,230,115,154,173,248,207,22,157,193,99,175,22,53,174,38,191,77,212,150,62,217,177,220,128,128,128,128,128,128,128,128],[249,2,13,48,185,2,9,249,2,6,1,130,98,202,185,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,64,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,128,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,208,69,247,225,155,36,136,146,75,151,249,193,69,181,229,29,13,137,90,101,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,137,27,39,73,35,139,39,255,88,233,81,8,142,85,176,77,231,29,195,116,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,17,116,101,115,116,108,111,99,97,108,46,116,101,115,116,110,101,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]],"skip_bridge_call":false}"#;
const DEPOSITED_RECIPIENT: &'static str = "testlocal.testnet";
const PROVER_ACCOUNT: &'static str = "eth_connector.root";
const CUSTODIAN_ADDRESS: &'static str = "d045f7e19B2488924B97F9c145b5E51D0D895A65";
const DEPOSITED_AMOUNT: u128 = 800400;
const DEPOSITED_FEE: u128 = 400;
const RECIPIENT_ETH_ADDRESS: &'static str = "891b2749238b27ff58e951088e55b04de71dc374";
const EVM_CUSTODIAN_ADDRESS: &'static str = "d045f7e19b2488924b97f9c145b5e51d0d895a65";
const DEPOSITED_EVM_AMOUNT: u128 = 800400;
const DEPOSITED_EVM_FEE: u128 = 400;

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
    pub skip_bridge_call: bool,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct InitCallArgs {
    pub prover_account: String,
    pub eth_custodian_address: String,
}

fn init(custodian_address: &str) -> (UserAccount, UserAccount) {
    let master_account = near_sdk_sim::init_simulator(None);
    let contract_account = master_account.deploy(
        *EVM_WASM_BYTES,
        CONTRACT_ACC.to_string(),
        to_yocto("1000000"),
    );
    contract_account
        .call(
            CONTRACT_ACC.to_string(),
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
            CONTRACT_ACC.to_string(),
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
    (master_account, contract_account)
}

fn validate_eth_address(address: &str) -> EthAddress {
    let data = hex::decode(address).unwrap();
    assert_eq!(data.len(), 20);
    let mut result = [0u8; 20];
    result.copy_from_slice(&data);
    result
}

fn call_deposit_near(master_account: &UserAccount) {
    let proof: Proof = serde_json::from_str(PROOF_DATA_NEAR).unwrap();
    let res = master_account.call(
        CONTRACT_ACC.to_string(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        0,
    );
    res.assert_success();
    //println!("{:#?}", res.promise_results());
}

#[allow(dead_code)]
fn print_logs(logs: &Vec<String>) {
    for l in logs {
        println!("[log] {}", l);
    }
}

fn call_deposit_eth(master_account: &UserAccount) {
    let proof: Proof = serde_json::from_str(PROOF_DATA_ETH).unwrap();
    let res = master_account.call(
        CONTRACT_ACC.to_string(),
        "deposit",
        &proof.try_to_vec().unwrap(),
        DEFAULT_GAS,
        10,
    );
    res.assert_success();
    //println!("{:#?}", res.promise_results());
}

fn get_near_balance(master_account: &UserAccount, acc: &str) -> u128 {
    #[derive(BorshSerialize)]
    pub struct BalanceOfCallArgs {
        pub account_id: String,
    }

    let balance = master_account.view(
        CONTRACT_ACC.to_string(),
        "ft_balance_of",
        &BalanceOfCallArgs {
            account_id: acc.into(),
        }
        .try_to_vec()
        .unwrap(),
    );
    String::from_utf8(balance.unwrap())
        .unwrap()
        .parse()
        .unwrap()
}

fn get_eth_balance(master_account: &UserAccount, address: EthAddress) -> u128 {
    #[derive(BorshSerialize, BorshDeserialize)]
    pub struct BalanceOfEthCallArgs {
        pub address: EthAddress,
    }

    let balance = master_account.view(
        CONTRACT_ACC.to_string(),
        "ft_balance_of_eth",
        &BalanceOfEthCallArgs { address }.try_to_vec().unwrap(),
    );
    String::from_utf8(balance.unwrap())
        .unwrap()
        .parse()
        .unwrap()
}

fn total_supply(master_account: &UserAccount) -> u128 {
    let balance = master_account.view(CONTRACT_ACC.to_string(), "ft_total_supply", &[]);
    String::from_utf8(balance.unwrap())
        .unwrap()
        .parse()
        .unwrap()
}

fn total_supply_near(master_account: &UserAccount) -> u128 {
    let balance = master_account.view(CONTRACT_ACC.to_string(), "ft_total_supply_near", &[]);
    String::from_utf8(balance.unwrap())
        .unwrap()
        .parse()
        .unwrap()
}

fn total_supply_eth(master_account: &UserAccount) -> u128 {
    let balance = master_account.view(CONTRACT_ACC.to_string(), "ft_total_supply_eth", &[]);
    String::from_utf8(balance.unwrap())
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn test_near_deposit_balance_total_supply() {
    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_near(&contract);

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = get_near_balance(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE);

    let balance = total_supply(&master_account);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_supply_near(&master_account);
    assert_eq!(balance, DEPOSITED_AMOUNT);

    let balance = total_supply_eth(&master_account);
    assert_eq!(balance, 0);
}

#[test]
fn test_eth_deposit_balance_total_supply() {
    let (master_account, contract) = init(EVM_CUSTODIAN_ADDRESS);
    call_deposit_eth(&contract);

    let balance = get_eth_balance(&master_account, validate_eth_address(RECIPIENT_ETH_ADDRESS));
    assert_eq!(balance, DEPOSITED_EVM_AMOUNT);

    let balance = total_supply(&master_account);
    assert_eq!(balance, DEPOSITED_EVM_AMOUNT);

    let balance = total_supply_eth(&master_account);
    assert_eq!(balance, DEPOSITED_EVM_AMOUNT);

    let balance = total_supply_near(&master_account);
    assert_eq!(balance, 0);
}

#[test]
fn test_withdraw_near() {
    #[derive(BorshSerialize, BorshDeserialize)]
    pub struct WithdrawCallArgs {
        pub recipient_id: String,
        pub amount: Balance,
    }

    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_near(&contract);

    let withdraw_amount = 100;
    let res = contract.call(
        CONTRACT_ACC.to_string(),
        "withdraw",
        &WithdrawCallArgs {
            recipient_id: RECIPIENT_ETH_ADDRESS.into(),
            amount: withdraw_amount,
        }
        .try_to_vec()
        .unwrap(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();

    let balance = get_near_balance(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE - withdraw_amount as u128);

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = total_supply(&master_account);
    assert_eq!(balance, DEPOSITED_AMOUNT - withdraw_amount as u128);
}

#[test]
fn test_ft_transfer() {
    #[derive(BorshSerialize, BorshDeserialize)]
    pub struct TransferCallArgs {
        pub receiver_id: String,
        pub amount: Balance,
        pub memo: Option<String>,
    }

    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_near(&contract);

    let transfer_amount = 70;
    let res = contract.call(
        CONTRACT_ACC.to_string(),
        "ft_transfer",
        &TransferCallArgs {
            receiver_id: DEPOSITED_RECIPIENT.into(),
            amount: transfer_amount,
            memo: None,
        }
        .try_to_vec()
        .unwrap(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT);
    assert_eq!(
        balance,
        DEPOSITED_AMOUNT - DEPOSITED_FEE + transfer_amount as u128
    );

    let balance = get_near_balance(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE - transfer_amount as u128);

    let balance = total_supply(&master_account);
    assert_eq!(balance, DEPOSITED_EVM_AMOUNT);

    let balance = total_supply_eth(&master_account);
    assert_eq!(balance, 0);

    let balance = total_supply_near(&master_account);
    assert_eq!(balance, DEPOSITED_AMOUNT);
}

/*
#[test]
fn test_ft_transfer_call() {
    let (master_account, _contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_near(&master_account);

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = get_near_balance(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE);

    let transfer_amount = 100;
    let res = master_account.call(
        CONTRACT_ACC.to_string(),
        "ft_transfer_call",
        json!({
            "receiver_id": CONTRACT_ACC,
            "amount": transfer_amount,
            "memo": "transfer memo",
            "msg": "some message"
        })
        .to_string()
        .as_bytes(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT);
    assert_eq!(
        balance,
        DEPOSITED_AMOUNT - DEPOSITED_FEE - transfer_amount as u128
    );

    let balance = get_near_balance(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE + transfer_amount as u128);
}
*/

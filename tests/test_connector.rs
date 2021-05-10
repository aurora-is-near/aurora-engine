#![allow(dead_code)]

use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::serde_json;
use near_sdk::test_utils::accounts;
use near_sdk_sim::{to_yocto, ExecutionResult, UserAccount, DEFAULT_GAS, STORAGE_AMOUNT};

use aurora_engine::parameters::NewCallArgs;
use aurora_engine::types::{Balance, EthAddress};
use byte_slice_cast::AsByteSlice;
use near_sdk_sim::transaction::ExecutionStatus;
use primitive_types::U256;

const CONTRACT_ACC: &'static str = "eth_connector.root";
const EXTERNAL_CONTRACT_ACC: &'static str = "eth_recipient.root";
const PROOF_DATA_NEAR: &'static str = r#"{"log_index":0,"log_entry_data":[248,251,148,208,69,247,225,155,36,136,146,75,151,249,193,69,181,229,29,13,137,90,101,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,137,27,39,73,35,139,39,255,88,233,81,8,142,85,176,77,231,29,195,116,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"receipt_index":0,"receipt_data":[249,2,6,1,130,98,214,185,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,64,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,128,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,208,69,247,225,155,36,136,146,75,151,249,193,69,181,229,29,13,137,90,101,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,137,27,39,73,35,139,39,255,88,233,81,8,142,85,176,77,231,29,195,116,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"header_data":[249,2,12,160,102,166,216,90,249,113,19,154,192,123,231,73,72,196,109,178,111,87,24,184,77,224,31,222,203,163,83,46,31,10,152,43,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,242,208,170,209,213,87,125,27,67,170,77,108,7,250,150,14,95,185,72,147,160,137,203,214,211,135,51,122,241,224,192,99,143,5,175,60,50,48,16,91,79,30,234,202,0,238,225,35,173,175,9,255,207,160,249,6,155,103,84,64,218,62,146,22,213,216,147,200,45,35,251,112,156,10,248,160,1,51,149,35,84,11,204,144,224,202,160,57,88,18,64,136,9,46,94,250,29,211,240,5,167,101,181,222,218,72,245,140,165,214,183,59,172,200,197,244,43,114,203,185,1,0,0,32,0,0,0,0,0,0,4,0,0,32,128,0,128,0,0,0,0,32,0,0,0,0,128,1,16,0,0,0,0,0,0,0,0,1,0,0,0,0,18,128,0,2,0,32,8,0,0,0,0,0,0,0,0,0,0,0,16,0,0,0,0,0,0,0,0,0,0,0,1,0,0,2,0,8,0,16,0,32,0,0,0,8,0,0,0,0,0,0,8,0,0,0,0,0,0,0,0,0,10,0,32,0,0,0,0,2,0,0,8,0,0,34,0,0,0,0,0,0,0,0,0,144,0,0,32,0,0,0,8,0,0,0,0,0,64,64,0,0,0,0,0,0,0,0,0,32,0,0,0,8,0,0,0,64,0,0,128,64,0,0,16,0,0,0,0,1,64,0,0,0,0,0,2,18,0,0,0,16,0,0,0,16,0,16,0,0,0,4,0,0,128,0,0,2,0,0,0,32,0,0,0,0,0,0,32,0,0,64,0,64,0,0,128,16,0,0,0,0,2,0,32,16,0,0,68,0,0,0,0,129,0,0,0,0,2,0,128,8,0,0,0,128,0,8,16,8,0,0,0,4,0,0,0,0,132,146,162,104,46,131,154,200,13,131,122,18,29,131,12,132,130,132,96,139,224,9,142,68,117,98,98,97,32,119,97,115,32,104,101,114,101,160,3,77,225,44,138,47,145,239,76,233,166,87,199,16,138,239,111,218,83,244,238,103,225,253,101,162,63,83,80,97,14,44,136,210,143,251,125,3,6,84,139],"proof":[[248,81,160,101,193,98,201,122,99,79,150,77,201,152,125,142,203,159,193,180,191,202,17,225,169,97,183,162,211,201,36,49,254,236,143,128,128,128,128,128,128,128,160,234,163,244,31,238,12,182,10,192,199,135,253,80,240,8,202,13,199,117,5,77,122,34,235,11,193,102,240,148,211,231,117,128,128,128,128,128,128,128,128],[249,2,13,48,185,2,9,249,2,6,1,130,98,214,185,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,64,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,128,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,248,253,248,251,148,208,69,247,225,155,36,136,146,75,151,249,193,69,181,229,29,13,137,90,101,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,137,27,39,73,35,139,39,255,88,233,81,8,142,85,176,77,231,29,195,116,184,160,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,18,101,116,104,95,114,101,99,105,112,105,101,110,116,46,114,111,111,116,0,0,0,0,0,0,0,0,0,0,0,0,0,0]]}"#;
const PROOF_DATA_ETH: &'static str = r#"{"log_index":0,"log_entry_data":[249,1,27,148,208,69,247,225,155,36,136,146,75,151,249,193,69,181,229,29,13,137,90,101,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,137,27,39,73,35,139,39,255,88,233,81,8,142,85,176,77,231,29,195,116,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"receipt_index":1,"receipt_data":[249,2,41,1,131,24,182,98,185,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,64,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,128,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,208,69,247,225,155,36,136,146,75,151,249,193,69,181,229,29,13,137,90,101,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,137,27,39,73,35,139,39,255,88,233,81,8,142,85,176,77,231,29,195,116,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0],"header_data":[249,2,30,160,111,97,25,111,90,125,206,227,215,193,148,45,147,4,187,198,152,166,152,186,159,210,49,186,75,34,150,201,54,105,5,168,160,29,204,77,232,222,199,93,122,171,133,181,103,182,204,212,26,211,18,69,27,148,138,116,19,240,161,66,253,64,212,147,71,148,124,28,230,160,8,239,64,193,62,78,177,68,166,204,116,240,224,174,172,126,160,111,195,78,89,67,41,2,157,170,245,45,186,44,22,233,68,147,196,225,10,188,79,39,185,164,159,1,50,218,63,126,149,160,29,232,191,170,186,241,208,229,220,99,239,186,250,187,144,97,103,177,34,12,215,67,242,112,214,72,49,13,103,43,51,100,160,54,149,161,219,243,80,47,227,85,72,213,8,136,187,146,242,175,109,136,59,112,7,18,70,53,231,137,106,131,174,238,206,185,1,0,64,36,66,0,0,0,0,0,0,0,16,144,136,128,0,4,0,0,5,0,68,18,69,0,130,133,0,1,72,20,1,6,0,36,0,0,0,1,0,0,66,130,0,3,0,32,2,16,64,0,8,20,16,0,18,0,0,0,64,0,0,32,0,0,4,32,64,0,8,8,17,24,0,2,0,8,80,16,4,164,2,1,0,88,33,16,2,0,0,0,8,0,128,128,32,0,8,144,64,33,10,8,0,0,0,2,0,0,0,20,40,0,8,16,17,0,32,136,0,0,64,40,66,16,0,193,36,65,8,0,0,0,0,0,0,24,9,68,0,4,0,64,4,2,1,0,0,33,128,0,0,8,0,64,0,65,8,0,144,128,2,64,4,0,0,0,0,1,64,0,0,1,0,0,128,18,0,0,18,16,0,1,4,17,0,50,0,0,4,0,16,8,48,16,1,2,17,0,32,33,36,68,1,0,134,10,32,32,0,68,64,64,0,0,0,176,4,80,130,96,3,8,160,0,0,16,38,36,0,16,0,129,66,64,16,4,6,1,129,8,34,32,40,136,1,21,0,64,18,0,36,4,0,18,0,0,132,146,254,52,83,131,154,200,22,131,122,18,0,131,96,14,161,132,96,139,224,101,160,124,155,151,179,209,252,221,180,120,228,141,224,208,96,198,37,67,148,68,112,98,116,99,115,116,48,48,51,1,2,188,2,160,159,177,10,214,36,197,89,9,154,67,118,246,150,110,69,238,177,236,63,15,238,7,125,131,200,52,124,15,61,45,216,54,136,49,112,168,92,196,85,129,181],"proof":[[248,113,160,44,213,237,238,173,98,115,100,91,239,4,240,232,47,186,18,143,197,102,238,1,53,102,75,9,209,147,160,7,21,161,20,160,130,179,147,241,60,191,220,0,36,63,44,21,155,22,112,231,108,237,85,245,123,92,87,5,27,18,188,251,63,49,48,62,128,128,128,128,128,128,160,249,62,174,85,169,158,2,131,251,223,51,75,126,80,21,56,49,223,181,2,186,104,110,128,183,35,245,41,213,86,163,142,128,128,128,128,128,128,128,128],[249,1,241,128,160,210,140,224,69,210,124,24,23,116,105,10,90,238,125,241,136,217,5,88,224,66,48,171,16,220,4,61,241,179,36,81,107,160,7,40,112,108,98,192,25,248,251,60,206,145,220,150,244,87,81,137,47,128,52,30,61,168,173,102,1,107,92,186,113,143,160,7,226,155,85,111,40,81,43,247,194,244,110,27,66,29,166,98,140,114,187,88,58,121,91,112,78,246,184,42,120,197,30,160,49,79,219,132,178,116,241,6,50,152,17,20,206,250,152,166,251,107,49,45,238,91,15,1,140,66,131,42,214,116,66,15,160,45,47,50,113,95,28,133,139,149,60,17,12,112,195,130,150,85,182,174,121,128,217,237,193,52,38,10,48,245,35,19,139,160,206,100,194,214,25,182,189,234,230,27,181,97,132,77,62,81,54,159,28,157,173,187,248,253,21,177,108,87,151,86,19,32,160,82,201,125,55,90,83,205,76,249,131,46,145,215,203,47,114,237,153,30,26,63,232,143,87,39,255,118,232,111,184,108,0,160,74,187,146,93,207,201,155,190,164,93,242,44,198,219,19,185,179,149,71,222,75,45,49,10,165,127,66,42,168,189,107,2,160,145,131,79,228,45,239,229,22,254,197,77,129,226,56,79,112,98,247,83,129,128,227,168,245,140,47,137,64,99,38,213,47,160,79,231,156,116,97,11,104,234,162,62,97,63,59,180,46,236,13,86,221,173,155,19,111,82,128,22,231,13,33,39,254,187,160,104,244,244,189,36,213,74,98,219,132,239,197,245,11,137,138,142,70,138,103,136,4,130,208,53,15,140,90,26,101,159,25,160,75,58,146,188,128,44,110,52,154,66,237,66,75,26,0,184,217,136,244,45,72,253,69,4,39,141,28,31,23,31,28,42,160,147,80,106,12,203,236,237,153,116,44,25,94,201,48,60,39,103,186,53,238,195,226,127,153,233,209,247,142,214,39,191,152,160,147,173,221,1,104,20,114,155,4,35,86,254,52,140,150,239,28,218,112,35,111,216,37,240,175,195,217,185,134,243,141,4,160,233,39,134,241,220,66,71,176,75,145,59,59,81,40,18,231,176,144,84,138,137,225,244,203,66,239,63,210,8,160,207,209,128],[249,2,48,32,185,2,44,249,2,41,1,131,24,182,98,185,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,32,0,0,0,0,0,0,0,0,0,0,128,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,64,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,128,8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,249,1,30,249,1,27,148,208,69,247,225,155,36,136,146,75,151,249,193,69,181,229,29,13,137,90,101,248,66,160,209,66,67,156,39,142,37,218,217,165,7,102,241,83,208,227,210,215,191,43,209,111,194,120,28,75,212,148,178,177,90,157,160,0,0,0,0,0,0,0,0,0,0,0,0,137,27,39,73,35,139,39,255,88,233,81,8,142,85,176,77,231,29,195,116,184,192,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,96,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,12,54,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,144,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59,101,116,104,95,99,111,110,110,101,99,116,111,114,46,114,111,111,116,58,56,57,49,66,50,55,52,57,50,51,56,66,50,55,102,70,53,56,101,57,53,49,48,56,56,101,53,53,98,48,52,100,101,55,49,68,99,51,55,52,0,0,0,0,0]]}"#;
const DEPOSITED_RECIPIENT: &'static str = "eth_recipient.root";
const PROVER_ACCOUNT: &'static str = "eth_connector.root";
const CUSTODIAN_ADDRESS: &'static str = "d045f7e19B2488924B97F9c145b5E51D0D895A65";
const DEPOSITED_AMOUNT: u128 = 800400;
const DEPOSITED_FEE: u128 = 400;
const RECIPIENT_ETH_ADDRESS: &'static str = "891b2749238b27ff58e951088e55b04de71dc374";
const EVM_CUSTODIAN_ADDRESS: &'static str = "d045f7e19b2488924b97f9c145b5e51d0d895a65";
const DEPOSITED_EVM_AMOUNT: u128 = 800400;
const DEPOSITED_EVM_FEE: u128 = 400;

near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
    EVM_WASM_BYTES => "test.wasm"
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
    assert_eq!(balance, DEPOSITED_EVM_AMOUNT - DEPOSITED_FEE);

    let balance = get_eth_balance(
        &master_account,
        validate_eth_address(CUSTODIAN_ADDRESS),
        CONTRACT_ACC,
    );
    assert_eq!(balance, DEPOSITED_FEE);

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

    let (master_account, contract) = init(CUSTODIAN_ADDRESS);
    call_deposit_near(&contract, CONTRACT_ACC);

    let withdraw_amount = 100;
    let res = contract.call(
        CONTRACT_ACC.to_string(),
        "withdraw",
        &WithdrawCallArgs {
            recipient_address: validate_eth_address(RECIPIENT_ETH_ADDRESS),
            amount: withdraw_amount,
        }
        .try_to_vec()
        .unwrap(),
        DEFAULT_GAS,
        1,
    );
    res.assert_success();

    let balance = get_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE - withdraw_amount as u128);

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT - DEPOSITED_FEE);

    let balance = total_supply(&master_account, CONTRACT_ACC);
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
    call_deposit_near(&contract, CONTRACT_ACC);

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

    let balance = get_near_balance(&master_account, DEPOSITED_RECIPIENT, CONTRACT_ACC);
    assert_eq!(
        balance,
        DEPOSITED_AMOUNT - DEPOSITED_FEE + transfer_amount as u128
    );

    let balance = get_near_balance(&master_account, CONTRACT_ACC, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_FEE - transfer_amount as u128);

    let balance = total_supply(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_EVM_AMOUNT);

    let balance = total_supply_eth(&master_account, CONTRACT_ACC);
    assert_eq!(balance, 0);

    let balance = total_supply_near(&master_account, CONTRACT_ACC);
    assert_eq!(balance, DEPOSITED_AMOUNT);
}

#[test]
fn test_ft_transfer_call_erc20() {
    #[derive(BorshSerialize)]
    pub struct TransferCallCallArgs {
        pub receiver_id: String,
        pub amount: Balance,
        pub memo: Option<String>,
        pub msg: String,
    }

    #[derive(BorshSerialize, BorshDeserialize)]
    pub struct DeployEvmTokenCallArgs {
        pub near_account_id: String,
        pub erc20_contract: Vec<u8>,
    }

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
        &TransferCallCallArgs {
            receiver_id: CONTRACT_ACC.into(),
            amount: transfer_amount,
            memo: None,
            msg: message,
        }
        .try_to_vec()
        .unwrap(),
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

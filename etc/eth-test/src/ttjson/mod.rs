use std::fs::read_to_string;

// helper function to read file
pub fn read_file(path: String) -> String {
    return read_to_string(path).unwrap();
}

pub mod tt_json_with_error;
pub mod tt_json_with_success; 

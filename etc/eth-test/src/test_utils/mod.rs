use std::fs::read_to_string;

// helper function to read file
fn read_file(path: String) -> String {
    return read_to_string(path).unwrap();
}

 mod address_less_than_20;
 mod address_less_than_20_prefixed;
 mod address_more_than_20;

pub use address_less_than_20::AddressLessThan20;
pub use address_less_than_20_prefixed::AddressLessThan20Prefixed0;
pub use address_more_than_20::AddressMoreThan20;
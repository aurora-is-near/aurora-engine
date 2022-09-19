use crate::test_utils::TransactionTestJson;
use std::fs::read_to_string;
use std::path::Path;

fn parse_test_transaction(json_str: String) -> Result<TransactionTestJson, std::io::Error>{
   serde_json::from_str(&json_str).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

fn read_file(path: String) -> Result<String, std::io::Error> {
    return read_to_string(path);
}

#[test]
fn test_parsing() -> Result<(), std::io::Error> {
    let json_str = read_file("TransactionTests/ttAddress/AddressLessThan20.json".to_string())?;
    println!("{:?}", json_str);
    let tt = parse_test_transaction(json_str)?;
    println!("{:?}", tt);
    Ok(())
}
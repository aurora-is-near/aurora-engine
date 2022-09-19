use crate::test_utils::AddressLessThan20Prefixed0;
use crate::test_utils::AddressLessThan20;
use crate::test_utils::AddressMoreThan20;
use std::fs::read_to_string;
use std::path::Path;



#[test]
fn test_address_less_than_20() -> Result<(), std::io::Error>  {
    let address_less_than_20 = AddressLessThan20::new();
    println!("{:?}", address_less_than_20.info());
    println!("{:?}", address_less_than_20.result("London".to_string()));
    println!("{:?}", address_less_than_20.txbytes());
    Ok(())
}

#[test]
fn test_address_less_than_20_prefixed() -> Result<(), std::io::Error>  {
    let address_less_than_20_prefixed = AddressLessThan20Prefixed0::new();
    println!("{:?}", address_less_than_20_prefixed.info());
    println!("{:?}", address_less_than_20_prefixed.result("London".to_string()));
    println!("{:?}", address_less_than_20_prefixed.txbytes()); 
    Ok(())
}

#[test]
fn test_address_more_than_20() -> Result<(), std::io::Error>  {
    let address_more_than_20 = AddressMoreThan20::new();
    println!("{:?}", address_more_than_20.info());
    println!("{:?}", address_more_than_20.result("London".to_string()));
    println!("{:?}", address_more_than_20.txbytes()); 
    Ok(())
}
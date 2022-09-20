use crate::ttjson::tt_json_with_error::{TransactionTestJsonErr, TransactionTestErr, TransactonTestInfo, TtResultErr, TTErr};
use crate::ttjson::tt_json_with_success::{TransactionTestJsonOk, TransactionTestOk, TtResultOk, TTOk};

#[test]
fn test_address_less_than_20() -> Result<(), std::io::Error> {
    let address_less_than_20 = TransactionTestErr::new("TransactionTests/ttAddress/AddressLessThan20.json".to_string(), "AddressLessThan20".to_string());
    println!("{:?}", address_less_than_20.info());
    println!("{:?}", address_less_than_20.result("London".to_string()));
    println!("{:?}", address_less_than_20.txbytes());
    Ok(())
}

#[test]
fn test_address_less_than_20_prefixed() -> Result<(), std::io::Error>  {
    let address_less_than_20_prefixed = TransactionTestOk::new("TransactionTests/ttAddress/AddressLessThan20Prefixed0.json".to_string(), "AddressLessThan20Prefixed0".to_string());
    println!("{:?}", address_less_than_20_prefixed.info());
    println!("{:?}", address_less_than_20_prefixed.result("London".to_string()));
    println!("{:?}", address_less_than_20_prefixed.txbytes()); 
    Ok(())
}

#[test]
fn test_address_more_than_20() -> Result<(), std::io::Error>  {
    let address_more_than_20 = TransactionTestErr::new("TransactionTests/ttAddress/AddressMoreThan20.json".to_string(), "AddressMoreThan20".to_string());
    println!("{:?}", address_more_than_20.info());
    println!("{:?}", address_more_than_20.result("London".to_string()));
    println!("{:?}", address_more_than_20.txbytes()); 
    Ok(())
}

#[test]
fn test_address_more_than_20_prefixed_by_0() -> Result<(), std::io::Error>  {
    let address_more_than_20_prefixed_by_0 = TransactionTestErr::new("TransactionTests/ttAddress/AddressMoreThan20PrefixedBy0.json".to_string(), "AddressMoreThan20PrefixedBy0".to_string());
    println!("{:?}", address_more_than_20_prefixed_by_0.info());
    println!("{:?}", address_more_than_20_prefixed_by_0.result("London".to_string()));
    println!("{:?}", address_more_than_20_prefixed_by_0.txbytes()); 
    Ok(())
}

#[test]
fn test_data_test_enough_gas()-> Result<(), std::io::Error> {
    let data_test_enough_gas = TransactionTestOk::new("TransactionTests/ttData/DataTestEnoughGAS.json".to_string(), "DataTestEnoughGAS".to_string());
    println!("{:?}", data_test_enough_gas.info());
    println!("{:?}", data_test_enough_gas.result("London".to_string()));
    println!("{:?}", data_test_enough_gas.txbytes()); 
    Ok(())
}

#[test]
fn test_data_test_first_zero_bytes() -> Result<(), std::io::Error> {
    let data_test_first_zero_bytes = TransactionTestOk::new("TransactionTests/ttData/DataTestFirstZeroBytes.json".to_string(), "DataTestFirstZeroBytes".to_string());
    println!("{:?}", data_test_first_zero_bytes.info());
    println!("{:?}", data_test_first_zero_bytes.result("London".to_string()));
    println!("{:?}", data_test_first_zero_bytes.txbytes()); 
    Ok(())
}

#[test]
fn test_data_test_last_zero_bytes() -> Result<(), std::io::Error> {
    let data_test_last_zero_bytes = TransactionTestOk::new("TransactionTests/ttData/DataTestLastZeroBytes.json".to_string(), "DataTestLastZeroBytes".to_string());
    println!("{:?}", data_test_last_zero_bytes.info());
    println!("{:?}", data_test_last_zero_bytes.result("London".to_string()));
    println!("{:?}", data_test_last_zero_bytes.txbytes()); 
    Ok(())
}

#[test]
fn test_data_test_not_enough_gas() -> Result<(), std::io::Error> {
    let data_test_not_enough_gas = TransactionTestErr::new("TransactionTests/ttData/DataTestNotEnoughGAS.json".to_string(), "DataTestNotEnoughGAS".to_string());
    println!("{:?}", data_test_not_enough_gas.info());
    println!("{:?}", data_test_not_enough_gas.result("London".to_string()));
    println!("{:?}", data_test_not_enough_gas.txbytes()); 
    Ok(())
}

#[test]
fn test_data_test_zero_bytes() -> Result<(), std::io::Error> {
    let data_test_zero_bytes = TransactionTestOk::new("TransactionTests/ttData/DataTestZeroBytes.json".to_string(), "DataTestZeroBytes".to_string());
    println!("{:?}", data_test_zero_bytes.info());
    println!("{:?}", data_test_zero_bytes.result("London".to_string()));
    println!("{:?}", data_test_zero_bytes.txbytes()); 
    Ok(())
}

#[test]
fn test_dataTx_bcValidBlockTest() -> Result<(), std::io::Error> {
    let data_tx_bc_valid_block_test = TransactionTestOk::new("TransactionTests/ttData/dataTx_bcValidBlockTest.json".to_string(), "dataTx_bcValidBlockTest".to_string());
    println!("{:?}", data_tx_bc_valid_block_test.info());
    println!("{:?}", data_tx_bc_valid_block_test.result("London".to_string()));
    println!("{:?}", data_tx_bc_valid_block_test.txbytes()); 
    Ok(())
}

#[test]
fn test_dataTx_bcValidBlockTestFrontier() -> Result<(), std::io::Error> {
    let data_tx_bc_valid_block_test_frontier = TransactionTestOk::new("TransactionTests/ttData/dataTx_bcValidBlockTestFrontier.json".to_string(), "dataTx_bcValidBlockTestFrontier".to_string());
    println!("{:?}", data_tx_bc_valid_block_test_frontier.info());
    println!("{:?}", data_tx_bc_valid_block_test_frontier.result("London".to_string()));
    println!("{:?}", data_tx_bc_valid_block_test_frontier.txbytes()); 
    Ok(())
}


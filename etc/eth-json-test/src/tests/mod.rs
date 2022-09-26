use crate::transaction_test::{TransactionTestJson, TransactionTest};


#[test]
fn test_data_test_first_zero_bytes() -> Result<(), std::io::Error> {
    let data_test_first_zero_bytes = TransactionTest::new("tests/TransactionTests/ttData/DataTestFirstZeroBytes.json".to_string(), "DataTestFirstZeroBytes".to_string());
    println!("{:?}", data_test_first_zero_bytes.info());
    println!("{:?}", data_test_first_zero_bytes.result("London".to_string()));
    println!("{:?}", data_test_first_zero_bytes.txbytes()); 
    Ok(())
}

#[test]
fn test_data_test_last_zero_bytes() -> Result<(), std::io::Error> {
    let data_test_last_zero_bytes = TransactionTest::new("tests/TransactionTests/ttData/DataTestLastZeroBytes.json".to_string(), "DataTestLastZeroBytes".to_string());
    println!("{:?}", data_test_last_zero_bytes.info());
    println!("{:?}", data_test_last_zero_bytes.result("London".to_string()));
    println!("{:?}", data_test_last_zero_bytes.txbytes()); 
    Ok(())
}

#[test]
fn test_data_test_not_enough_gas() -> Result<(), std::io::Error> {
    let data_test_not_enough_gas = TransactionTest::new("tests/TransactionTests/ttData/DataTestNotEnoughGAS.json".to_string(), "DataTestNotEnoughGAS".to_string());
    println!("{:?}", data_test_not_enough_gas.info());
    println!("{:?}", data_test_not_enough_gas.result("London".to_string()));
    println!("{:?}", data_test_not_enough_gas.txbytes()); 
    Ok(())
}

#[test]
fn test_data_test_zero_bytes() -> Result<(), std::io::Error> {
    let data_test_zero_bytes = TransactionTest::new("tests/TransactionTests/ttData/DataTestZeroBytes.json".to_string(), "DataTestZeroBytes".to_string());
    println!("{:?}", data_test_zero_bytes.info());
    println!("{:?}", data_test_zero_bytes.result("London".to_string()));
    println!("{:?}", data_test_zero_bytes.txbytes()); 
    Ok(())
}

#[test]
fn test_dataTx_bcValidBlockTest() -> Result<(), std::io::Error> {
    let data_tx_bc_valid_block_test = TransactionTest::new("tests/TransactionTests/ttData/dataTx_bcValidBlockTest.json".to_string(), "dataTx_bcValidBlockTest".to_string());
    println!("{:?}", data_tx_bc_valid_block_test.info());
    println!("{:?}", data_tx_bc_valid_block_test.result("London".to_string()));
    println!("{:?}", data_tx_bc_valid_block_test.txbytes()); 
    Ok(())
}

// Tests on the test data json file where it provides inconsistent results for different networks

#[test]
fn test_dataTx_bcValidBlockTestFrontier() -> Result<(), std::io::Error> {
    let data_tx_bc_valid_block_test_frontier = TransactionTest::new("tests/TransactionTests/ttData/dataTx_bcValidBlockTestFrontier.json".to_string(), "dataTx_bcValidBlockTestFrontier".to_string());
    println!("{:?}", data_tx_bc_valid_block_test_frontier.info());
    println!("{:?}", data_tx_bc_valid_block_test_frontier.result("London".to_string()));
    println!("{:?}", data_tx_bc_valid_block_test_frontier.txbytes()); 
    Ok(())
}

#[test]
fn test_dataTx_bcValidBlockTestFrontierOnOtherNewtork() -> Result<(), std::io::Error> {
    let data_tx_bc_valid_block_test_frontier = TransactionTest::new("tests/TransactionTests/ttData/dataTx_bcValidBlockTestFrontier.json".to_string(), "dataTx_bcValidBlockTestFrontier".to_string());
    println!("{:?}", data_tx_bc_valid_block_test_frontier.info());
    println!("{:?}", data_tx_bc_valid_block_test_frontier.result("Frontier".to_string()));
    println!("{:?}", data_tx_bc_valid_block_test_frontier.txbytes()); 
    Ok(())
}



# Ethereum Transaction Test Parser
---

*This project is Rust typings for [ethereum test](https://github.com/ethereum/tests) for feeding inputs to EVM for rust developers*

## Getting started

Ethereum tests are divided with tests that **passes** with predictable hash with sender and **errors** with exception.

##### Typing in tx passing tests

If testing json file includes `hash` and `sender` in result of the transaction, Parse json file with `TransactionTestOk` parser.

```Rust
#[test]
fn test_address_less_than_20_prefixed() -> Result<(), std::io::Error>  {
    let address_less_than_20_prefixed = TransactionTestOk::new("TransactionTests/ttAddress/AddressLessThan20Prefixed0.json".to_string(), "AddressLessThan20Prefixed0".to_string());
    println!("{:?}", address_less_than_20_prefixed.info());
    println!("{:?}", address_less_than_20_prefixed.result("London".to_string()));
    println!("{:?}", address_less_than_20_prefixed.txbytes()); 
    Ok(())
}
```

##### Typing in tx failing tests

If testing json file includes `exception` with error string, Parse test json file with `TransacitonTestErr` parser.

```Rust
#[test]
fn test_address_more_than_20() -> Result<(), std::io::Error>  {
    let address_more_than_20 = TransactionTestErr::new("TransactionTests/ttAddress/AddressMoreThan20.json".to_string(), "AddressMoreThan20".to_string());
    println!("{:?}", address_more_than_20.info());
    println!("{:?}", address_more_than_20.result("London".to_string()));
    println!("{:?}", address_more_than_20.txbytes()); 
    Ok(())
}
```
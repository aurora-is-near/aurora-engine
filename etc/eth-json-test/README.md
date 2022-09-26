# Ethereum Transaction Test Parser
---

*This project is Rust JSON parser for [ethereum test](https://github.com/ethereum/tests) for feeding inputs to EVM*

## Getting started

Specify which transaction test's path and name to parse

```Rust
#[test]
fn test_address_less_than_20_prefixed() -> Result<(), std::io::Error>  {
    let address_less_than_20_prefixed = TransactionTest::new("tests/TransactionTests/ttAddress/AddressLessThan20Prefixed0.json".to_string(), "AddressLessThan20Prefixed0".to_string());
    println!("{:?}", address_less_than_20_prefixed.info());
    println!("{:?}", address_less_than_20_prefixed.result("London".to_string()));
    println!("{:?}", address_less_than_20_prefixed.txbytes()); 
    Ok(())
}
```
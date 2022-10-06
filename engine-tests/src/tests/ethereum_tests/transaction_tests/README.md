# Transaction Tests

Transaction tests are testing the Aurora engine parser to show correct exceptions on the RLP encoded malformed transaction.

Each transaction test file will include RLP txbytes which is a signed transaction.
The exception should match from the json result of each version of EVM

```
fn run(path: String, name: String) {
    let mut runner = initialize_runner();

    // Bring up the test json file
    let tt_json = TransactionTest::new(path, name);
    let tx_bytes_str = &tt_json.txbytes;
    let txbytes: Vec<u8> = hexstr_to_bytes(&tx_bytes_str);
    
    // Do transaction with tx bytes as data 
    let outcome:Result<(SubmitResult, ExecutionProfile), VMError> = runner.submit_transaction_raw(txbytes);
    match  tt_json.result("London".to_string()) {
        TtResult::TtResultOk { hash, intrinsic_gas, sender } => {
            let _ok = TtResultOk {
                hash,
                intrinsic_gas,
                sender
            };
            assert!(outcome.is_ok())
        }
        TtResult::TtResultErr { exception, intrinsic_gas } => {
            let _err = TtResultErr {
                exception,
                intrinsic_gas
            };
            assert!(outcome.is_err())
            // TODO: Add exceptions to engine and test reasons on transaction parser
            // assert_eq!(outcome.to_string(), exception);
        },
    };
}

// Testing individual file
#[test]
fn test_address_less_than_20_prefixed() {
    run("/Users/hyungsukkang/aurora/hyungsuk/aurora-engine/etc/eth-json-test/res/tests/TransactionTests/ttAddress/AddressLessThan20.json".to_string(), "AddressLessThan20".to_string())
}
```
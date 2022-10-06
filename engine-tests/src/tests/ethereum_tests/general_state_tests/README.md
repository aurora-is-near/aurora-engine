# General State Tests

General State tests are testing the Aurora engine's evm to see if state functions as expected from Ethereum test spec.

Each transaction test file will include RLP txbytes which is a signed transaction from a preset account.
The logs of each transaction should match from the json result of each version of EVM.

```
// Test individually
// cargo test --features mainnet-test  --package aurora-engine-tests --lib -- tests::ethereum_tests::general_state_tests --nocapture
#[test]
pub fn test_add() {
    run("../etc/eth-json-test/res/tests/GeneralStateTests/VMTests/vmArithmeticTest/add.json".to_string(), "add".to_string())
}
```
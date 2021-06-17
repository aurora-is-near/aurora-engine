# ETH connector

## Build
1. For production set in the Makefile
    ```   
       FEATURES = contract
    ```
    1.1. For **development and testing** set in the Makefile
    ```
       FEATURES = contract,integration-test
    ```
2. Build release: `$ make release`
3. Run tests: `$ cargo test` 
4. Deploying process is common for Aurora itself. Please reference [README.md](../README.md)

## Initialize eth-conenctor
With `near-cli` run:
```
$ near call <NEAR_ACC> new_eth_connector '{"prover_account": "<PROVER_NEAR_ACCOUNT>", "eth_custodian_address": "<ETH_ADDRESS>"}' --account-id <NEAR_ACC>

```

## ETH connector specific methods
* new_eth_connector (call once)
* deposit (mutable)
* withdraw (mutable, payable)
* finish_deposit (private, mutable)
* ft_total_supply (view)
* ft_total_eth_supply_on_near (view)
* ft_total_eth_supply_on_aurora (view)
* ft_balance_of (view)
* ft_balance_of_eth (view)
* ft_transfer (mutable, payable)
* ft_resolve_transfer (private, mutable)
* ft_transfer_call (mutable, payable)
* ft_on_transfer (private, mutable)
* storage_deposit (mutable)
* storage_withdraw (mutable, payable)
* storage_balance_of (view)

## Ethereum specific flow
Follow by [this instruction](https://github.com/aurora-is-near/eth-connector/blob/master/README.md).

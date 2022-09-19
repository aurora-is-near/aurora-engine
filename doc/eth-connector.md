# ETH connector

Aurora ETH connector is implementation for [NEP-141](https://nomicon.io/Standards/Tokens/FungibleToken/Core).

It has two basic accounts entities:
* Aurora on NEAR (`balance_of`)
* Aurora on ETH (`balance_of_eth`)

This means that there are two types of `total supply`:
* `total_eth_supply_on_near`
* `total_eth_supply_on_aurora`

Eth-Connector logic can be divided into three large groups:
1. NEP-141 specific logic
2. Eth-Connector specific logic
3. Admin Controlled specific logic

## ETH connector methods

* new_eth_connector (call once)
    > Initialize Eth Connector. Called once.

* verify_log_entry (integration-test, view)
   > Used for integrations tests only.

#### NEP-141 specific logic

For more details see: [NEP-141](https://nomicon.io/Standards/Tokens/FungibleToken/Core).

* ft_total_supply (view)
* ft_total_eth_supply_on_near (view)
* ft_total_eth_supply_on_aurora (view)
* ft_balance_of (view)
* ft_balance_of_eth (view)
* ft_transfer (mutable, payable)
* ft_resolve_transfer (private, mutable)
* storage_deposit (mutable)
* storage_withdraw (mutable, payable)
* storage_balance_of (view)
* ft_metadata (view)

* ft_transfer_call (mutable, payable)
   > - Verify message data if `sender_id == receiver_id ` before `ft_on_transfer` call to avoid verification panics
   >   - Fetch transfer message
   >   - Check is transfer amount > fee
   >   - Check overflow for recipient  `balance_of_eth_on_aurora` before process `ft_on_transfer`
   >   - Check overflow for `total_eth_supply_on_aurora` before process `ft_on_transfer`
   > - if sender_id != receiver_id
   >   - `transfer_eth_on_near` from `sender_id` to `receiver_id`
   > - Call `ft_on_transfer`
* ft_on_transfer (mutable)
   > - Fetch transfer message
   > - mint_eth_on_aurora for `recipient`
   > - if `fee` exist mint_eth_on_aurora for `relayer`

#### Eth-Connector specific logic

* deposit (mutable)
   > Deposit logic:
   > - fetch proof
   > - Prepare token message data for Finish Deposit
   > - Invoke promise - Verify proof log entry data by Custodian
   > - Invoke promise Finish Deposit with Token message data
   > 
   > Arguments: (proof: Proof)

* withdraw (mutable, payable)
   > Withdraw from NEAR accounts.
   > 
   > Arguments: (recipient_address: Address, amount: NEP141Wei) 

* finish_deposit (private, mutable)
   > Finish deposit logic 
   > - Check is Verify proof log entry data success
   > - If msg is set
   >   - Mint amount for Owner
   >   - Record Proof
   >   - Call ft_transfer_call for receiver_id
   > - else
   >   - Mint amount for Owner
   >   - Mint fee for relayer
   >   - Record Proof
   > 
   > Arguments: (deposit_call: FinishDepositCallArgs, [callback] verify_log_result: bool)

#### Admin Controlled specific logic

* get_accounts_counter (view)
* get_paused_flags (view)
* set_paused_flags (mutable, private)

## ETH connector specific source files

* `fungible_token.rs`
* `connector.rs`
* `admin_controlled.rs`
* `deposit_event.rs`
* `log_entry.rs`
* `proof.rs`

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

## Initialize eth-connector
With `near-cli` run:
```
$ near call <NEAR_ACC> new_eth_connector '{"prover_account": "<PROVER_NEAR_ACCOUNT>", "eth_custodian_address": "<ETH_ADDRESS>"}' --account-id <NEAR_ACC>

```

## Ethereum specific flow
Follow by [this instruction](https://github.com/aurora-is-near/eth-connector/blob/master/README.md).

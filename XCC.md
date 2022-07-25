# Cross contract call

## Engine interface

- Propose: Any user can propose a new contract to be used as proxy contract. It is stored on-chain. 
- Accept: Admin should accept some proposed contract. The current version is bumped!

## Engine storage

Maintain a list of all deployed contracts, with the version of the current deployed bytecode. Version is an integer bigger than 0.

## Host function

Whenever an updated is needed, a new proxy bytecode is proposed and accepted. Going forward before calling the proxy,
the new version will be deployed and initialized. 

Method init must be agnostic to the current state of the contract, it is possible updating a contract 
skipping an arbitrary number of versions. 

### Promise anatomy

When a call to xcc host function is made, a bundle is created using the concatenation of the following promises:

- create-account (only if account hasn't been created yet)
- deploy-contract (only if current deployed contract doesn't match current version)
- init-call (only if deploy contrac must be called)
- xcc-access (alwasy)
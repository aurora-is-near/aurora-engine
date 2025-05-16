# Contract Verification Guide

This guide explains how to verify your smart contract on Blockscout using Hardhat, starting from installing `@nomicfoundation/hardhat-verify` and `dotenv`.

## Steps

### 1. Install Required Packages

First, install the necessary packages:

```bash
npm install --save-dev @nomicfoundation/hardhat-verify dotenv
```

- **`@nomicfoundation/hardhat-verify`**: Plugin for verifying contracts.
- **`dotenv`**: For managing environment variables securely.

### 2. Create a `.env` File

Create a `.env` file under `/etc/eth-contracts` directory to store environment variables. This is already added to `.gitignore` to prevent sensitive data from being committed.

```bash
CHAIN_ID=1313161555
NETWORK_NAME=testnet
RPC_URL=https://testnet.aurora.dev
API_URL=https://explorer.testnet.aurora.dev/api
BROWSER_URL=https://explorer.testnet.aurora.dev
PRIVATE_KEY=your_private_key_here    # Required for deployment
```

**Note:**

- Replace `your_private_key_here` with your actual private key(required only for deployment).


### 3. Compile Your Contract

Compile your smart contract using Hardhat.

```bash
npx hardhat compile
```

### 4. Verify Your Contract

After compiling, verify your contract using the Hardhat verification plugin.

```bash
npx hardhat verify --network testnet DEPLOYED_CONTRACT_ADDRESS "Constructor Argument 1" "Constructor Argument 2"
```

**Replace:**

- `DEPLOYED_CONTRACT_ADDRESS` with your contract's address.
- `"Constructor Argument 1"`, `"Constructor Argument 2"` with any constructor arguments required by your contract.

**Example:**

```bash
npx hardhat verify --network testnet 0x4988a896b1227218e4A686fdE5EabdcAbd91571f  "Empty" "EMPTY" "0" "0xbdab39a000332777a778afe92e83a6c630bd9a38"
```
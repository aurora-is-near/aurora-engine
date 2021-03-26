# Scripts for Aurora Engine Development

## Prerequisites

- Node.js (v14+)

```shell
npm install
```

## Usage

### Deployment

```console
$ node deploy.js -h

Usage: deploy [options]

Options:
  -d, --debug                enable debug output
  --signer <ACCOUNT>         specify signer master account ID (default: "test.near")
  --evm <ACCOUNT>            specify EVM contract account ID (default: "evm.test.near")
  --chain <ID>               specify chain ID (default: 0)
  --owner <ACCOUNT>          specify owner account ID (default: "")
  --bridge-prover <ACCOUNT>  specify bridge prover account ID (default: "")
  --upgrade-delay <BLOCKS>   specify upgrade delay block count (default: 0)
  -h, --help                 display help for command
```

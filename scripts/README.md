# Aurora Command-Line Interface (CLI)

## Prerequisites

- Node.js (v14+)

```shell
npm install
```

## Reference

### `aurora help`

```console
$ node aurora.js help
Usage: aurora [options] [command]

Options:
  -d, --debug                                    enable debug output
  --signer <account>                             specify signer master account ID (default: "test.near")
  --evm <account>                                specify EVM contract account ID (default: "aurora.test.near")
  -h, --help                                     display help for command

Commands:
  init [options]
  get-version|get_version
  get-owner|get_owner
  get-bridge-provider|get_bridge_provider
  get-chain-id|get_chain_id
  get-upgrade-index|get_upgrade_index
  stage-upgrade|stage_upgrade
  deploy-upgrade|deploy_upgrade
  deploy-code|deploy_code <bytecode>
  call <address> <input>
  raw-call|raw_call <input>
  meta-call|meta_call
  view [options] <address> <input>
  get-code|get_code <address>
  get-balance|get_balance <address>
  get-nonce|get_nonce <address>
  get-storage-at|get_storage_at <address> <key>
  begin-chain|begin_chain <id>
  begin-block|begin_block <hash>
  help [command]                                 display help for command
```

### `aurora init`

```console
$ node aurora.js init -h
Usage: aurora init [options]

Options:
  --chain <id>               specify EVM chain ID (default: 0)
  --owner <account>          specify owner account ID (default: null)
  --bridge-prover <account>  specify bridge prover account ID (default: null)
  --upgrade-delay <blocks>   specify upgrade delay block count (default: 0)
  -h, --help                 display help for command
```

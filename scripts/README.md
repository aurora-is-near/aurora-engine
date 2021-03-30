# Scripts for Aurora Engine Development

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
  -d, --debug          enable debug output
  --signer <ACCOUNT>   specify signer master account ID (default: "test.near")
  --evm <ACCOUNT>      specify EVM contract account ID (default:
                       "evm.test.near")
  --chain <ID>         specify chain ID (default: 0)
  -h, --help           display help for command

Commands:
  init [options]
  get_version
  get_owner
  get_bridge_provider
  get_chain_id
  get_upgrade_index
  stage_upgrade
  deploy_upgrade
  deploy_code
  call
  raw_call
  meta_call
  view
  get_code
  get_balance
  get_nonce
  get_storage_at
  begin_chain
  begin_block
  help [command]       display help for command
```

### `aurora init`

```console
$ node aurora.js init -h
Usage: aurora init [options]

Options:
  --owner <ACCOUNT>          specify owner account ID (default: null)
  --bridge-prover <ACCOUNT>  specify bridge prover account ID (default: null)
  --upgrade-delay <BLOCKS>   specify upgrade delay block count (default: 0)
  -h, --help                 display help for command
```

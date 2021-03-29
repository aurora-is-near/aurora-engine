import { NewCallArgs } from '@aurora-is-near/engine';
import { defaultAbiCoder } from '@ethersproject/abi';
import { arrayify } from '@ethersproject/bytes';
import BN from 'bn.js';
import { program } from 'commander';
import nearAPI from 'near-api-js';

main(process.argv, process.env);

async function main(argv, env) {
  program
    .option('-d, --debug', 'enable debug output')
    .option("--signer <ACCOUNT>", "specify signer master account ID", env.NEAR_MASTER_ACCOUNT || 'test.near')
    .option("--evm <ACCOUNT>", "specify EVM contract account ID", env.NEAR_EVM_ACCOUNT || 'evm.test.near')
    .option("--chain <ID>", "specify chain ID", 0);

  program
    .command('get_version')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('get_owner')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('get_bridge_provider')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('get_chain_id')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('get_upgrade_index')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('stage_upgrade')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('deploy_upgrade')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('deploy_code')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('call')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('raw_call')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('meta_call')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('view')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('get_code')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('get_balance')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('get_nonce')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('get_storage_at')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('begin_chain')
    .action((_options, _command) => {
      // TODO
    });

  program
    .command('begin_block')
    .action((_options, _command) => {
      // TODO
    });

  program.parse(process.argv);
}

async function rawFunctionCall(signer, contractID, args) {
  const action = new nearAPI.transactions.Action({
    functionCall: new nearAPI.transactions.FunctionCall(args.toFunctionCall())
  });
  return signer.signAndSendTransaction(contractID, [action]);
}

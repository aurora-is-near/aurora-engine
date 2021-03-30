import { Engine } from '@aurora-is-near/engine';
import { program } from 'commander';

main(process.argv, process.env);

async function main(argv, env) {
  program
    .option('-d, --debug', 'enable debug output')
    .option("--signer <ACCOUNT>", "specify signer master account ID", env.NEAR_MASTER_ACCOUNT || 'test.near')
    .option("--evm <ACCOUNT>", "specify EVM contract account ID", env.NEAR_EVM_ACCOUNT || 'evm.test.near')
    .option("--chain <ID>", "specify chain ID", 0);

  program
    .command('get_version')
    .action(async (_options, command) => {
      const engine = await Engine.connect(command.parent.opts(), env);
      const result = await engine.getVersion();
      const version = result.toString('utf8', 0, result.length - 1);
      console.log(version);
    });

  program
    .command('get_owner')
    .action(async (_options, _command) => {
      const engine = await Engine.connect(command.parent.opts(), env);
      const result = await engine.getOwner();
      console.log(result);
    });

  program
    .command('get_bridge_provider')
    .action(async (_options, _command) => {
      const engine = await Engine.connect(command.parent.opts(), env);
      const result = await engine.getBridgeProvider();
      console.log(result);
    });

  program
    .command('get_chain_id')
    .action(async (options, command) => {
      const engine = await Engine.connect(command.parent.opts(), env);
      const chainID = await engine.getChainID();
      console.log(chainID);
    });

  program
    .command('get_upgrade_index')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('stage_upgrade')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('deploy_upgrade')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('deploy_code')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('call')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('raw_call')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('meta_call')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('view')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('get_code')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('get_balance')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('get_nonce')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('get_storage_at')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('begin_chain')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('begin_block')
    .action(async (_options, _command) => {
      // TODO
    });

  program.parse(process.argv);
}

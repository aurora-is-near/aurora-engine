import { Engine } from '@aurora-is-near/engine';
import { program } from 'commander';

main(process.argv, process.env);

async function main(argv, env) {
  program
    .option('-d, --debug', 'enable debug output')
    .option("--signer <account>", "specify signer master account ID", env.NEAR_MASTER_ACCOUNT || 'test.near')
    .option("--evm <account>", "specify EVM contract account ID", env.NEAR_EVM_ACCOUNT || 'evm.test.near')
    .option("--chain <id>", "specify EVM chain ID", 0);

  program
    .command('init')
    .option("--owner <account>", "specify owner account ID", null)
    .option("--bridge-prover <account>", "specify bridge prover account ID", null)
    .option("--upgrade-delay <blocks>", "specify upgrade delay block count", 0)
    .action(async (options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const outcome = await engine.initialize(config);
      if (config.debug) console.debug("Outcome:", outcome);
    });

  program
    .command('get-version')
    .alias('get_version')
    .action(async (options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const result = await engine.getVersion();
      const version = result.toString('utf8', 0, result.length - 1);
      console.log(version);
    });

  program
    .command('get-owner')
    .alias('get_owner')
    .action(async (options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const result = await engine.getOwner();
      const accountID = result.toString('utf8');
      console.log(accountID);
    });

  program
    .command('get-bridge-provider')
    .alias('get_bridge_provider')
    .action(async (options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const result = await engine.getBridgeProvider();
      const accountID = result.toString('utf8');
      console.log(accountID);
    });

  program
    .command('get-chain-id')
    .alias('get_chain_id')
    .alias('get-chain')
    .alias('get_chain')
    .action(async (options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const chainID = await engine.getChainID();
      console.log(chainID);
    });

  program
    .command('get-upgrade-index')
    .alias('get_upgrade_index')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('stage-upgrade')
    .alias('stage_upgrade')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('deploy-upgrade')
    .alias('deploy_upgrade')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('deploy-code')
    .alias('deploy_code')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('call')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('raw-call')
    .alias('raw_call')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('meta-call')
    .alias('meta_call')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('view')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('get-code <address>')
    .alias('get_code')
    .action(async (address, options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const code = await engine.getCode(address);
      console.log(`0x${code ? code.toString('hex') : ''}`);
    });

  program
    .command('get-balance <address>')
    .alias('get_balance')
    .action(async (address, options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const balance = await engine.getBalance(address);
      console.log(balance);
    });

  program
    .command('get-nonce <address>')
    .alias('get_nonce')
    .action(async (address, options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const nonce = await engine.getNonce(address);
      console.log(nonce);
    });

  program
    .command('get-storage-at <address> <key>')
    .alias('get_storage_at')
    .action(async (address, key, options, command) => {
      // TODO
    });

  program
    .command('begin-chain <id>')
    .alias('begin_chain')
    .action(async (chain_id, options, command) => {
      // TODO
    });

  program
    .command('begin-block <hash>')
    .alias('begin_block')
    .action(async (hash, options, command) => {
      // TODO
    });

  program.parse(process.argv);
}

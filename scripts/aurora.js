import { Engine } from '@aurora-is-near/engine';
import { program } from 'commander';
import { readFileSync } from 'fs';

main(process.argv, process.env);

async function main(argv, env) {
  program
    .option('-d, --debug', 'enable debug output')
    .option("--signer <account>", "specify signer master account ID", env.NEAR_MASTER_ACCOUNT || 'test.near')
    .option("--evm <account>", "specify EVM contract account ID", env.NEAR_EVM_ACCOUNT || 'aurora.test.near');

  program
    .command('init')
    .option("--chain <id>", "specify EVM chain ID", 0)
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
      const version = result.substring(0, result.length - 1);
      console.log(version);
    });

  program
    .command('get-owner')
    .alias('get_owner')
    .action(async (options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const accountID = await engine.getOwner();
      console.log(accountID);
    });

  program
    .command('get-bridge-provider')
    .alias('get_bridge_provider')
    .action(async (options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const accountID = await engine.getBridgeProvider();
      console.log(accountID);
    });

  program
    .command('get-chain-id')
    .aliases(['get_chain_id', 'get-chain', 'get_chain'])
    .action(async (options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const chainID = await engine.getChainID();
      console.log(chainID.toString());
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
    .command('deploy-code <bytecode>')
    .alias('deploy_code')
    .action(async (input, options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const address = await engine.deployCode(readInput(input));
      console.log(address);
    });

  program
    .command('call <address> <input>')
    .action(async (address, input, options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const output = await engine.call(readInput(address), readInput(input));
      console.log(`0x${output ? output.toString('hex') : ''}`);
    });

  program
    .command('raw-call <input>')
    .alias('raw_call')
    .action(async (_input, _options, _command) => {
      // TODO
    });

  program
    .command('meta-call') // TODO
    .alias('meta_call')
    .action(async (_options, _command) => {
      // TODO
    });

  program
    .command('view <address> <input>')
    .option("--sender <address>", "specify the sender address", '0x0000000000000000000000000000000000000000') // TODO
    .option("--amount <value>", "attach an ETH amount", 0)
    .action(async (address, input, options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const output = await engine.view(options.sender, readInput(address), BigInt(config.amount), readInput(input));
      console.log(`0x${output ? output.toString('hex') : ''}`);
    });

  program
    .command('get-code <address>')
    .alias('get_code')
    .action(async (address, options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const code = await engine.getCode(readInput(address));
      console.log(`0x${code ? code.toString('hex') : ''}`);
    });

  program
    .command('get-balance <address>')
    .alias('get_balance')
    .action(async (address, options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const balance = await engine.getBalance(readInput(address));
      console.log(balance.toString());
    });

  program
    .command('get-nonce <address>')
    .alias('get_nonce')
    .action(async (address, options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const nonce = await engine.getNonce(readInput(address));
      console.log(nonce.toString());
    });

  program
    .command('get-storage-at <address> <key>')
    .aliases(['get_storage_at', 'get-storage', 'get_storage'])
    .action(async (address, key, options, command) => {
      const config = {...command.parent.opts(), ...options};
      if (config.debug) console.debug("Options:", config);
      const engine = await Engine.connect(config, env);
      const value = await engine.getStorageAt(readInput(address), key);
      console.log(value.toString());
    });

  program
    .command('begin-chain <id>')
    .alias('begin_chain')
    .action(async (chain_id, options, command) => {
      // TODO
    });

  program
    .command('begin-block <hash>') // TODO
    .alias('begin_block')
    .action(async (hash, options, command) => {
      // TODO
    });

  program.parse(process.argv);
}

function readInput(input) {
  try {
    return (input[0] == '@') ? readFileSync(input.substring(1), 'ascii').trim() : input;
  } catch (err) {
    console.error(err.toString());
    process.exit(-1);
  }
}

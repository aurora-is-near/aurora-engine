import { Engine } from '@aurora-is-near/engine';
import { program } from 'commander';

main(process.argv, process.env);

async function main(argv, env) {
  const options = program
    .option('-d, --debug', 'enable debug output')
    .option("--signer <ACCOUNT>", "specify signer master account ID", env.NEAR_MASTER_ACCOUNT || 'test.near')
    .option("--evm <ACCOUNT>", "specify EVM contract account ID", env.NEAR_EVM_ACCOUNT || 'evm.test.near')
    .option("--chain <ID>", "specify chain ID", 0)
    .option("--owner <ACCOUNT>", "specify owner account ID", '')
    .option("--bridge-prover <ACCOUNT>", "specify bridge prover account ID", '')
    .option("--upgrade-delay <BLOCKS>", "specify upgrade delay block count", 0)
    .parse(process.argv)
    .opts();
  if (options.debug) console.debug(options);

  const engine = await Engine.connect(options, env);

  const outcome = await engine.initialize(options);
  if (options.debug) console.debug(outcome);
}

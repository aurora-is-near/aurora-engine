import { NewCallArgs } from '@aurora-is-near/engine';
import { defaultAbiCoder } from '@ethersproject/abi';
import { arrayify } from '@ethersproject/bytes';
import BN from 'bn.js';
import { program } from 'commander';
import nearAPI from 'near-api-js';

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

  const near = await nearAPI.connect({
    deps: {keyStore: new nearAPI.keyStores.InMemoryKeyStore()},
    networkId: env.NEAR_ENV || 'local',
    nodeUrl: 'http://localhost:3030',
    keyPath: `${env.HOME}/.near/validator_key.json`,
    contractName: options.evm,
  });
  const signer = await near.account(options.signer);

  let newCall = new NewCallArgs(
    arrayify(defaultAbiCoder.encode(['uint256'], [options.chain])),
    options.owner,
    options.bridgeProver,
    options.upgradeDelay
  );
  if (options.debug) console.debug(newCall);

  let outcome = await rawFunctionCall(signer, options.evm, newCall);
  if (options.debug) console.debug(outcome);
}

async function rawFunctionCall(signer, contractID, args) {
  const action = new nearAPI.transactions.Action({
    functionCall: new nearAPI.transactions.FunctionCall(args.toFunctionCall())
  });
  return signer.signAndSendTransaction(contractID, [action]);
}

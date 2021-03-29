/* This is free and unencumbered software released into the public domain. */

import { NewCallArgs } from './schema.js';
import { defaultAbiCoder } from '@ethersproject/abi';
import { arrayify } from '@ethersproject/bytes';
import BN from 'bn.js';
import NEAR from 'near-api-js';

export class Engine {
  constructor(
    public near: NEAR.Near,
    public signer: NEAR.Account) {}

  static async connect(options: any, env: any): Promise<Engine> {
    const near = await NEAR.connect({
      deps: {keyStore: new NEAR.keyStores.InMemoryKeyStore()},
      networkId: env.NEAR_ENV || 'local',
      nodeUrl: 'http://localhost:3030',
      keyPath: `${env.HOME}/.near/validator_key.json`,
    });
    const signer = await near.account(options.signer);
    return new Engine(near, signer);
  }

  async initialize(options: any): Promise<any> {
    const args = new NewCallArgs(
      arrayify(defaultAbiCoder.encode(['uint256'], [options.chain])),
      options.owner,
      options.bridgeProver,
      options.upgradeDelay
    );
    return await this.signer!.functionCall(options.evm, 'new', args.encode());
  }

  async getChainID(): Promise<BN> {
    return new BN(0);
  }
}

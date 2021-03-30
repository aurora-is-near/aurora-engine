/* This is free and unencumbered software released into the public domain. */

import { NewCallArgs } from './schema.js';
import { defaultAbiCoder } from '@ethersproject/abi';
import { arrayify } from '@ethersproject/bytes';
import BN from 'bn.js';
import NEAR from 'near-api-js';

export class Engine {
  constructor(
    public near: NEAR.Near,
    public signer: NEAR.Account,
    public contract: string) {}

  static async connect(options: any, env: any): Promise<Engine> {
    const near = await NEAR.connect({
      deps: {keyStore: new NEAR.keyStores.InMemoryKeyStore()},
      networkId: env.NEAR_ENV || 'local',
      nodeUrl: 'http://localhost:3030',
      keyPath: `${env.HOME}/.near/validator_key.json`,
    });
    const signer = await near.account(options.signer);
    return new Engine(near, signer, options.evm);
  }

  async initialize(options: any): Promise<any> {
    const args = new NewCallArgs(
      arrayify(defaultAbiCoder.encode(['uint256'], [options.chain])),
      options.owner,
      options.bridgeProver,
      options.upgradeDelay
    );
    return await this.signer!.functionCall(this.contract, 'new', args.encode());
  }

  async getVersion(): Promise<string> {
    return await this.signer!.viewFunction(this.contract, 'get_version', {}, { parse: (x: any): any => x });
  }

  async getOwner(): Promise<string> {
    return await this.signer!.viewFunction(this.contract, 'get_owner', {}, { parse: (x: any): any => x });
  }

  async getBridgeProvider(): Promise<string> {
    return await this.signer!.viewFunction(this.contract, 'get_bridge_provider', {}, { parse: (x: any): any => x });
  }

  async getChainID(): Promise<BN> {
    const result = await this.signer!.viewFunction(this.contract, 'get_chain_id', {}, { parse: (x: any): any => x });
    return result.readUInt32BE(28);
  }
}

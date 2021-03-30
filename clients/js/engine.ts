/* This is free and unencumbered software released into the public domain. */

import { NewCallArgs } from './schema.js';
import { defaultAbiCoder } from '@ethersproject/abi';
import { getAddress } from '@ethersproject/address';
import { arrayify } from '@ethersproject/bytes';
import { toBigIntBE } from 'bigint-buffer';
import BN from 'bn.js';
import NEAR from 'near-api-js';

const noParse = { parse: (x: any): any => x };

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
      arrayify(defaultAbiCoder.encode(['uint256'], [options.chain || 0])),
      options.owner || '',
      options.bridgeProver || '',
      new BN(options.upgradeDelay || 0)
    );
    return await this.signer!.functionCall(this.contract, 'new', args.encode());
  }

  async getVersion(): Promise<string> {
    return await this.signer!.viewFunction(this.contract, 'get_version', {}, noParse);
  }

  async getOwner(): Promise<string> {
    return await this.signer!.viewFunction(this.contract, 'get_owner', {}, noParse);
  }

  async getBridgeProvider(): Promise<string> {
    return await this.signer!.viewFunction(this.contract, 'get_bridge_provider', {}, noParse);
  }

  async getChainID(): Promise<bigint> {
    const result = await this.signer!.viewFunction(this.contract, 'get_chain_id', {}, noParse);
    return toBigIntBE(result);
  }

  async getCode(address: string): Promise<Buffer> {
    const args = arrayify(getAddress(address));
    const result = await this.signer!.viewFunction(this.contract, 'get_code', args, noParse);
    return result;
  }

  async getBalance(address: string): Promise<bigint> {
    const args = arrayify(getAddress(address));
    const result = await this.signer!.viewFunction(this.contract, 'get_balance', args, noParse);
    return toBigIntBE(result);
  }

  async getNonce(address: string): Promise<bigint> {
    const args = arrayify(getAddress(address));
    const result = await this.signer!.viewFunction(this.contract, 'get_nonce', args, noParse);
    return toBigIntBE(result);
  }
}

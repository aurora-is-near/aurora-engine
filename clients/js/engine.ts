/* This is free and unencumbered software released into the public domain. */

import { GetStorageAtArgs, NewCallArgs } from './schema.js';
import { defaultAbiCoder } from '@ethersproject/abi';
import { getAddress } from '@ethersproject/address';
import { arrayify } from '@ethersproject/bytes';
import { toBigIntBE } from 'bigint-buffer';
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
      arrayify(defaultAbiCoder.encode(['uint256'], [options.chain || 0])),
      options.owner || '',
      options.bridgeProver || '',
      new BN(options.upgradeDelay || 0)
    );
    return await this.callMutativeFunction('new', args.encode());
  }

  async getVersion(): Promise<string> {
    return await this.callFunction('get_version');
  }

  async getOwner(): Promise<string> {
    return await this.callFunction('get_owner');
  }

  async getBridgeProvider(): Promise<string> {
    return await this.callFunction('get_bridge_provider');
  }

  async getChainID(): Promise<bigint> {
    const result = await this.callFunction('get_chain_id');
    return toBigIntBE(result);
  }

  async getCode(address: string): Promise<Uint8Array> {
    const args = arrayify(getAddress(address));
    return await this.callFunction('get_code', args);
  }

  async getBalance(address: string): Promise<bigint> {
    const args = arrayify(getAddress(address));
    const result = await this.callFunction('get_balance', args);
    return toBigIntBE(result);
  }

  async getNonce(address: string): Promise<bigint> {
    const args = arrayify(getAddress(address));
    const result = await this.callFunction('get_nonce', args);
    return toBigIntBE(result);
  }

  async getStorageAt(address: string, key: string): Promise<Uint8Array> {
    const args = new GetStorageAtArgs(
      arrayify(getAddress(address)),
      arrayify(defaultAbiCoder.encode(['uint256'], [key])),
    );
    return await this.callFunction('get_storage_at', args.encode());
  }

  async callFunction(methodName: string, args: Uint8Array | null = null): Promise<any> {
    const result = await this.signer.connection.provider.query({
      request_type: 'call_function',
      account_id: this.contract,
      method_name: methodName,
      args_base64: (args ? Buffer.from(args!) : Buffer.alloc(0)).toString('base64'),
      finality: 'optimistic',
    });
    if (result.logs && result.logs.length > 0) console.debug(result.logs); // TODO
    return Buffer.from(result.result);
  }

  async callMutativeFunction(methodName: string, args: Uint8Array | null = null): Promise<any> {
    return await this.signer.functionCall(this.contract, methodName, args || Buffer.alloc(0));
  }
}

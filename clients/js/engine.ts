/* This is free and unencumbered software released into the public domain. */

import { FunctionCallArgs, GetStorageAtArgs, NewCallArgs, ViewCallArgs } from './schema.js';
import { defaultAbiCoder } from '@ethersproject/abi';
import { getAddress as parseAddress } from '@ethersproject/address';
import { arrayify as parseHexString } from '@ethersproject/bytes';
import { toBigIntBE, toBufferBE } from 'bigint-buffer';
import BN from 'bn.js';
import NEAR from 'near-api-js';

export { getAddress as parseAddress } from '@ethersproject/address';
export { arrayify as parseHexString } from '@ethersproject/bytes';

export type AccountID = string;
export type Address = string;
export type Amount = bigint | number;
export type Bytecode = Uint8Array;
export type Bytecodeish = Bytecode | string;
export type ChainID = bigint;
export type U256 = bigint;

export class Engine {
  constructor(
    public near: NEAR.Near,
    public signer: NEAR.Account,
    public contract: AccountID) {}

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
      parseHexString(defaultAbiCoder.encode(['uint256'], [options.chain || 0])),
      options.owner || '',
      options.bridgeProver || '',
      new BN(options.upgradeDelay || 0)
    );
    return await this.callMutativeFunction('new', args.encode());
  }

  async getVersion(): Promise<string> {
    return (await this.callFunction('get_version')).toString();
  }

  async getOwner(): Promise<AccountID> {
    return (await this.callFunction('get_owner')).toString();
  }

  async getBridgeProvider(): Promise<AccountID> {
    return (await this.callFunction('get_bridge_provider')).toString();
  }

  async getChainID(): Promise<ChainID> {
    const result = await this.callFunction('get_chain_id');
    return toBigIntBE(result);
  }

  async deployCode(bytecode: Bytecodeish): Promise<Address> {
    const args = parseHexString(bytecode);
    const result = await this.callMutativeFunction('deploy_code', args);
    return parseAddress(result.toString('hex'));
  }

  async call(contract: Address, input: Uint8Array | string): Promise<Uint8Array> {
    const args = new FunctionCallArgs(
      parseHexString(parseAddress(contract)),
      this.prepareInput(input),
    );
    return (await this.callMutativeFunction('call', args.encode()));
  }

  async view(sender: Address, address: Address, amount: Amount, input: Uint8Array | string): Promise<Uint8Array> {
    const args = new ViewCallArgs(
      parseHexString(parseAddress(sender)),
      parseHexString(parseAddress(address)),
      toBufferBE(BigInt(amount), 32),
      this.prepareInput(input),
    );
    return (await this.callFunction('view', args.encode()));
  }

  async getCode(address: Address): Promise<Bytecode> {
    const args = parseHexString(parseAddress(address));
    return await this.callFunction('get_code', args);
  }

  async getBalance(address: Address): Promise<U256> {
    const args = parseHexString(parseAddress(address));
    const result = await this.callFunction('get_balance', args);
    return toBigIntBE(result);
  }

  async getNonce(address: Address): Promise<U256> {
    const args = parseHexString(parseAddress(address));
    const result = await this.callFunction('get_nonce', args);
    return toBigIntBE(result);
  }

  async getStorageAt(address: Address, key: U256 | number | string): Promise<U256> {
    const args = new GetStorageAtArgs(
      parseHexString(parseAddress(address)),
      parseHexString(defaultAbiCoder.encode(['uint256'], [key])),
    );
    const result = await this.callFunction('get_storage_at', args.encode());
    return toBigIntBE(result);
  }

  protected async callFunction(methodName: string, args?: Uint8Array): Promise<Buffer> {
    const result = await this.signer.connection.provider.query({
      request_type: 'call_function',
      account_id: this.contract,
      method_name: methodName,
      args_base64: this.prepareInput(args).toString('base64'),
      finality: 'optimistic',
    });
    if (result.logs && result.logs.length > 0)
      console.debug(result.logs); // TODO
    return Buffer.from(result.result);
  }

  protected async callMutativeFunction(methodName: string, args?: Uint8Array): Promise<Buffer> {
    const result = await this.signer.functionCall(this.contract, methodName, this.prepareInput(args));
    if (typeof result.status === 'object' && typeof result.status.SuccessValue === 'string') {
      return Buffer.from(result.status.SuccessValue, 'base64');
    }
    throw new Error(result.toString()); // TODO
  }

  private prepareInput(args?: Uint8Array | string): Buffer {
    if (typeof args === 'undefined')
      return Buffer.alloc(0);
    if (typeof args === 'string')
      return Buffer.from(parseHexString(args as string));
    return Buffer.from(args);
  }
}

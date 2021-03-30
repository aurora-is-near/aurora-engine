/// <reference types="node" />
import NEAR from 'near-api-js';
export { getAddress as parseAddress } from '@ethersproject/address';
export { arrayify as parseHexString } from '@ethersproject/bytes';
export declare type AccountID = string;
export declare type Address = string;
export declare type Amount = bigint | number;
export declare type Bytecode = Uint8Array;
export declare type Bytecodeish = Bytecode | string;
export declare type ChainID = bigint;
export declare type U256 = bigint;
export declare class Engine {
    near: NEAR.Near;
    signer: NEAR.Account;
    contract: AccountID;
    constructor(near: NEAR.Near, signer: NEAR.Account, contract: AccountID);
    static connect(options: any, env: any): Promise<Engine>;
    initialize(options: any): Promise<any>;
    getVersion(): Promise<string>;
    getOwner(): Promise<AccountID>;
    getBridgeProvider(): Promise<AccountID>;
    getChainID(): Promise<ChainID>;
    deployCode(bytecode: Bytecodeish): Promise<Address>;
    call(contract: Address, input: Uint8Array | string): Promise<Uint8Array>;
    view(sender: Address, address: Address, amount: Amount, input: Uint8Array | string): Promise<Uint8Array>;
    getCode(address: Address): Promise<Bytecode>;
    getBalance(address: Address): Promise<U256>;
    getNonce(address: Address): Promise<U256>;
    getStorageAt(address: Address, key: U256 | number | string): Promise<U256>;
    protected callFunction(methodName: string, args?: Uint8Array): Promise<Buffer>;
    protected callMutativeFunction(methodName: string, args?: Uint8Array): Promise<Buffer>;
    private prepareInput;
}

import NEAR from 'near-api-js';
export declare class Engine {
    near: NEAR.Near;
    signer: NEAR.Account;
    contract: string;
    constructor(near: NEAR.Near, signer: NEAR.Account, contract: string);
    static connect(options: any, env: any): Promise<Engine>;
    initialize(options: any): Promise<any>;
    getVersion(): Promise<string>;
    getOwner(): Promise<string>;
    getBridgeProvider(): Promise<string>;
    getChainID(): Promise<bigint>;
    getCode(address: string): Promise<Uint8Array>;
    getBalance(address: string): Promise<bigint>;
    getNonce(address: string): Promise<bigint>;
    getStorageAt(address: string, key: string): Promise<Uint8Array>;
    viewFunction(methodName: string, args?: Uint8Array | null): Promise<any>;
}

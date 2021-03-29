import BN from 'bn.js';
import NEAR from 'near-api-js';
export declare class Engine {
    near: NEAR.Near;
    signer: NEAR.Account;
    constructor(near: NEAR.Near, signer: NEAR.Account);
    static connect(options: any, env: any): Promise<Engine>;
    initialize(options: any): Promise<any>;
    getChainID(): Promise<BN>;
}

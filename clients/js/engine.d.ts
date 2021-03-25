declare abstract class Assignable {
    constructor(properties: any);
    encode(): Uint8Array;
}
export declare class NewCallArgs extends Assignable {
    chainID: Uint8Array;
    ownerID: string;
    bridgeProverID: string;
    upgradeDelayBlocks: number;
    constructor(chainID: Uint8Array, ownerID: string, bridgeProverID: string, upgradeDelayBlocks: number);
}
export {};

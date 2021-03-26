declare abstract class Assignable {
    abstract functionName(): string;
    encode(): Uint8Array;
    toFunctionCall(): object;
}
export declare class NewCallArgs extends Assignable {
    chainID: Uint8Array;
    ownerID: string;
    bridgeProverID: string;
    upgradeDelayBlocks: number;
    constructor(chainID: Uint8Array, ownerID: string, bridgeProverID: string, upgradeDelayBlocks: number);
    functionName(): string;
}
export declare class MetaCallArgs extends Assignable {
    constructor();
    functionName(): string;
}
export declare class FunctionCallArgs extends Assignable {
    constructor();
    functionName(): string;
}
export declare class ViewCallArgs extends Assignable {
    constructor();
    functionName(): string;
}
export declare class GetStorageAtArgs extends Assignable {
    constructor();
    functionName(): string;
}
export declare class BeginChainArgs extends Assignable {
    constructor();
    functionName(): string;
}
export declare class BeginBlockArgs extends Assignable {
    constructor();
    functionName(): string;
}
export {};

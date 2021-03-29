import BN from 'bn.js';
declare abstract class Assignable {
    abstract functionName(): string;
    encode(): Uint8Array;
    toFunctionCall(): object;
}
export declare class NewCallArgs extends Assignable {
    chainID: Uint8Array;
    ownerID: string;
    bridgeProverID: string;
    upgradeDelayBlocks: number | BN;
    constructor(chainID: Uint8Array, ownerID: string, bridgeProverID: string, upgradeDelayBlocks: number | BN);
    functionName(): string;
}
export declare class GetChainID extends Assignable {
    constructor();
    functionName(): string;
}
export declare class MetaCallArgs extends Assignable {
    signature: Uint8Array;
    v: number;
    nonce: Uint8Array;
    feeAmount: Uint8Array;
    feeAddress: Uint8Array;
    contractAddress: Uint8Array;
    value: Uint8Array;
    methodDef: string;
    args: Uint8Array;
    constructor(signature: Uint8Array, v: number, nonce: Uint8Array, feeAmount: Uint8Array, feeAddress: Uint8Array, contractAddress: Uint8Array, value: Uint8Array, methodDef: string, args: Uint8Array);
    functionName(): string;
}
export declare class FunctionCallArgs extends Assignable {
    contract: Uint8Array;
    input: Uint8Array;
    constructor(contract: Uint8Array, input: Uint8Array);
    functionName(): string;
}
export declare class ViewCallArgs extends Assignable {
    sender: Uint8Array;
    address: Uint8Array;
    amount: Uint8Array;
    input: Uint8Array;
    constructor(sender: Uint8Array, address: Uint8Array, amount: Uint8Array, input: Uint8Array);
    functionName(): string;
}
export declare class GetStorageAtArgs extends Assignable {
    address: Uint8Array;
    key: Uint8Array;
    constructor(address: Uint8Array, key: Uint8Array);
    functionName(): string;
}
export declare class BeginChainArgs extends Assignable {
    chainID: Uint8Array;
    constructor(chainID: Uint8Array);
    functionName(): string;
}
export declare class BeginBlockArgs extends Assignable {
    hash: Uint8Array;
    coinbase: Uint8Array;
    timestamp: Uint8Array;
    number: Uint8Array;
    difficulty: Uint8Array;
    gaslimit: Uint8Array;
    constructor(hash: Uint8Array, coinbase: Uint8Array, timestamp: Uint8Array, number: Uint8Array, difficulty: Uint8Array, gaslimit: Uint8Array);
    functionName(): string;
}
export {};

/* This is free and unencumbered software released into the public domain. */
import BN from 'bn.js';
import NEAR from 'near-api-js';
class Assignable {
    encode() {
        return NEAR.utils.serialize.serialize(SCHEMA, this);
    }
    toFunctionCall() {
        return {
            methodName: this.functionName(),
            args: this.encode(),
            gas: new BN('300000000000000'),
            deposit: new BN(0),
        };
    }
}
// Borsh-encoded parameters for the `new` method.
export class NewCallArgs extends Assignable {
    constructor(chainID, ownerID, bridgeProverID, upgradeDelayBlocks) {
        super();
        this.chainID = chainID;
        this.ownerID = ownerID;
        this.bridgeProverID = bridgeProverID;
        this.upgradeDelayBlocks = upgradeDelayBlocks;
    }
    functionName() {
        return 'new';
    }
}
// Borsh-encoded parameters for the `get_chain_id` method.
export class GetChainID extends Assignable {
    constructor() { super(); }
    functionName() {
        return 'get_chain_id';
    }
}
// Borsh-encoded parameters for the `meta_call` method.
export class MetaCallArgs extends Assignable {
    constructor(signature, v, nonce, feeAmount, feeAddress, contractAddress, value, methodDef, args) {
        super();
        this.signature = signature;
        this.v = v;
        this.nonce = nonce;
        this.feeAmount = feeAmount;
        this.feeAddress = feeAddress;
        this.contractAddress = contractAddress;
        this.value = value;
        this.methodDef = methodDef;
        this.args = args;
    }
    functionName() {
        return 'meta_call';
    }
}
// Borsh-encoded parameters for the `call` method.
export class FunctionCallArgs extends Assignable {
    constructor(contract, input) {
        super();
        this.contract = contract;
        this.input = input;
    }
    functionName() {
        return 'call';
    }
}
// Borsh-encoded parameters for the `view` method.
export class ViewCallArgs extends Assignable {
    constructor(sender, address, amount, input) {
        super();
        this.sender = sender;
        this.address = address;
        this.amount = amount;
        this.input = input;
    }
    functionName() {
        return 'view';
    }
}
// Borsh-encoded parameters for the `get_storage_at` method.
export class GetStorageAtArgs extends Assignable {
    constructor(address, key) {
        super();
        this.address = address;
        this.key = key;
    }
    functionName() {
        return 'get_storage_at';
    }
}
// Borsh-encoded parameters for the `begin_chain` method.
export class BeginChainArgs extends Assignable {
    constructor(chainID) {
        super();
        this.chainID = chainID;
    }
    functionName() {
        return 'begin_chain';
    }
}
// Borsh-encoded parameters for the `begin_block` method.
export class BeginBlockArgs extends Assignable {
    constructor(hash, coinbase, timestamp, number, difficulty, gaslimit) {
        super();
        this.hash = hash;
        this.coinbase = coinbase;
        this.timestamp = timestamp;
        this.number = number;
        this.difficulty = difficulty;
        this.gaslimit = gaslimit;
    }
    functionName() {
        return 'begin_block';
    }
}
const SCHEMA = new Map([
    [NewCallArgs, { kind: 'struct', fields: [
                ['chainID', [32]],
                ['ownerID', 'string'],
                ['bridgeProverID', 'string'],
                ['upgradeDelayBlocks', 'u64'],
            ] }],
    [GetChainID, { kind: 'struct', fields: [] }],
    [MetaCallArgs, { kind: 'struct', fields: [
                ['signature', [64]],
                ['v', 'u8'],
                ['nonce', [32]],
                ['feeAmount', [32]],
                ['feeAddress', [20]],
                ['contractAddress', [20]],
                ['value', [32]],
                ['methodDef', 'string'],
                ['args', ['u8']],
            ] }],
    [FunctionCallArgs, { kind: 'struct', fields: [
                ['contract', [20]],
                ['input', ['u8']],
            ] }],
    [ViewCallArgs, { kind: 'struct', fields: [
                ['sender', [20]],
                ['address', [20]],
                ['amount', [32]],
                ['input', ['u8']],
            ] }],
    [GetStorageAtArgs, { kind: 'struct', fields: [
                ['address', [20]],
                ['key', [32]],
            ] }],
    [BeginChainArgs, { kind: 'struct', fields: [
                ['chainID', [32]],
            ] }],
    [BeginBlockArgs, { kind: 'struct', fields: [
                ['hash', [32]],
                ['coinbase', [32]],
                ['timestamp', [32]],
                ['number', [32]],
                ['difficulty', [32]],
                ['gaslimit', [32]],
            ] }],
]);
